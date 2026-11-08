import {
  createEffect,
  createSignal,
  createMemo,
  on,
  onCleanup,
  onMount,
  Show,
  For,
  Match,
  Switch,
} from "solid-js";
import { createStore, produce } from "solid-js/store";
import { useNavigate, useParams, A } from "@solidjs/router";
import {
  ScrapeJobStatus,
  StepStatus,
  type ScrapeJobResponse,
  type StepState,
} from "ramekin-client";

import { useAuth } from "../context/AuthContext";
import { usePageTitle } from "../utils/pageTitle";
import { extractApiError } from "../utils/recipeFormHelpers";
import {
  PollScrapeJobAbortedError,
  pollScrapeJob,
} from "../utils/pollScrapeJob";

/** A step's expanded output: loading, failed to load, or loaded. */
type ExpandedOutput =
  | { state: "loading" }
  | { state: "error"; message: string }
  | { state: "loaded"; output: unknown };

/** What the page shows: nothing yet, an error with no job, or the job. */
type PageView =
  | { kind: "loading" }
  | { kind: "error"; message: string }
  | { kind: "job"; job: ScrapeJobResponse; pollError: string | null };

function formatDuration(ms: number | null | undefined): string {
  if (ms == null) return "";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function stepIcon(status: StepStatus): string {
  switch (status) {
    case StepStatus.Completed:
      return "✓"; // ✓
    case StepStatus.Failed:
      return "✗"; // ✗
    case StepStatus.Running:
      return "◐"; // ◐
    case StepStatus.Pending:
      return "○"; // ○
  }
}

function statusLabel(status: ScrapeJobStatus): string {
  switch (status) {
    case ScrapeJobStatus.Pending:
      return "Pending";
    case ScrapeJobStatus.Scraping:
      return "Scraping";
    case ScrapeJobStatus.Parsing:
      return "Parsing";
    case ScrapeJobStatus.Normalizing:
      return "Normalizing";
    case ScrapeJobStatus.Completed:
      return "Completed";
    case ScrapeJobStatus.Failed:
      return "Failed";
  }
}

export default function ScrapeStatusPage() {
  usePageTitle(() => "Scrape Status");
  const params = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { getScrapeApi } = useAuth();

  const [job, setJob] = createSignal<ScrapeJobResponse | null>(null);
  const [pollError, setPollError] = createSignal<string | null>(null);
  const [retrying, setRetrying] = createSignal(false);
  const [now, setNow] = createSignal(Date.now());
  const [expanded, setExpanded] = createStore<Record<string, ExpandedOutput>>(
    {},
  );
  // The output request each expanded step is waiting on. A response whose
  // request is no longer current (the step was collapsed, a retry cleared
  // the outputs, or the page moved to another job) is dropped.
  const outputRequests = new Map<string, symbol>();

  let tickInterval: ReturnType<typeof setInterval> | null = null;
  let pollController: AbortController | null = null;

  const clearExpanded = () => {
    outputRequests.clear();
    setExpanded(
      produce((s) => {
        for (const key of Object.keys(s)) {
          delete s[key];
        }
      }),
    );
  };

  const startPolling = (id: string) => {
    pollController?.abort();
    const controller = new AbortController();
    pollController = controller;
    void pollScrapeJob(getScrapeApi(), id, {
      signal: controller.signal,
      timeoutMs: null,
      onUpdate: (resp) => {
        setJob(resp);
        setPollError(null);
      },
      onPollError: async (err) => {
        const message = await extractApiError(err, "Failed to load scrape");
        if (!controller.signal.aborted) setPollError(message);
      },
    })
      .then((result) => {
        if (result.status === "timeout") {
          setPollError("Timed out waiting for scrape job");
        }
      })
      .catch(async (err: unknown) => {
        if (err instanceof PollScrapeJobAbortedError) return;
        const message = await extractApiError(err, "Failed to load scrape");
        if (!controller.signal.aborted) setPollError(message);
      });
  };

  // The route reuses this component across /scrape/:id changes, so follow
  // the id rather than only polling on mount.
  createEffect(
    on(
      () => params.id,
      (id) => {
        setJob(null);
        setPollError(null);
        setRetrying(false);
        clearExpanded();
        if (id) {
          startPolling(id);
        } else {
          pollController?.abort();
          pollController = null;
        }
      },
    ),
  );

  onMount(() => {
    tickInterval = setInterval(() => setNow(Date.now()), 1000);
  });

  onCleanup(() => {
    pollController?.abort();
    pollController = null;
    outputRequests.clear();
    if (tickInterval) {
      clearInterval(tickInterval);
      tickInterval = null;
    }
  });

  const toggleExpand = async (step: StepState) => {
    const id = params.id;
    if (!id || !step.hasOutput) return;
    const name = step.name;
    if (expanded[name]) {
      outputRequests.delete(name);
      setExpanded(
        produce((s) => {
          delete s[name];
        }),
      );
      return;
    }
    const request = Symbol(name);
    outputRequests.set(name, request);
    const current = () => outputRequests.get(name) === request;
    setExpanded(name, { state: "loading" });
    try {
      const output = await getScrapeApi().getStepOutput({
        id,
        stepName: name,
      });
      if (current()) setExpanded(name, { state: "loaded", output });
    } catch (err) {
      const message = await extractApiError(err, "Failed to load step output");
      if (current()) setExpanded(name, { state: "error", message });
    }
  };

  const onRetry = async () => {
    const id = params.id;
    if (!id) return;
    setRetrying(true);
    try {
      await getScrapeApi().retryScrape({ id });
      if (params.id !== id) return;
      setPollError(null);
      // Clear any expanded outputs since we're starting over.
      clearExpanded();
      startPolling(id);
    } catch (err) {
      const message = await extractApiError(err, "Failed to retry scrape");
      if (params.id === id) setPollError(message);
    } finally {
      if (params.id === id) setRetrying(false);
    }
  };

  const view = createMemo((): PageView => {
    const j = job();
    if (j) return { kind: "job", job: j, pollError: pollError() };
    const message = pollError();
    if (message) return { kind: "error", message };
    return { kind: "loading" };
  });

  const elapsed = createMemo(() => {
    const j = job();
    if (!j) return "";
    const start =
      j.createdAt instanceof Date
        ? j.createdAt.getTime()
        : new Date(j.createdAt).getTime();
    const diff = Math.max(0, now() - start);
    if (diff < 1000) return `${diff}ms`;
    const seconds = diff / 1000;
    if (seconds < 60) return `${seconds.toFixed(1)}s`;
    const minutes = Math.floor(seconds / 60);
    const remSec = Math.floor(seconds % 60);
    return `${minutes}m ${remSec}s`;
  });

  const jobView = () => {
    const v = view();
    return v.kind === "job" ? v : null;
  };
  const errorView = () => {
    const v = view();
    return v.kind === "error" ? v : null;
  };

  return (
    <div class="scrape-status-page">
      <Switch>
        <Match when={errorView()}>
          {(v) => (
            <div class="error-state">
              <p class="error">{v().message}</p>
              <A href="/" class="btn">
                Back to Cookbook
              </A>
            </div>
          )}
        </Match>
        <Match when={view().kind === "loading"}>
          <p class="loading">Loading scrape…</p>
        </Match>
        <Match when={jobView()}>
          {(v) => {
            const j = () => v().job;
            return (
              <>
                <header class="scrape-status-header">
                  <h2>Scrape</h2>
                  <Show when={j().url}>
                    <div class="scrape-url">{j().url}</div>
                  </Show>
                  <div class="scrape-status-meta">
                    <span class="status-pill" data-status={j().status}>
                      {statusLabel(j().status)}
                    </span>
                    <span class="elapsed">{elapsed()}</span>
                  </div>
                </header>

                <Show when={v().pollError}>
                  <div class="scrape-poll-error">{v().pollError}</div>
                </Show>

                <Show when={j().status === ScrapeJobStatus.Failed && j().error}>
                  <div class="error-banner">
                    <div>
                      <strong>
                        Failed
                        <Show when={j().failedAtStep}>
                          {" "}
                          at {j().failedAtStep}
                        </Show>
                        :
                      </strong>{" "}
                      {j().error}
                    </div>
                    <Show when={j().canRetry}>
                      <button
                        type="button"
                        class="btn btn-primary"
                        onClick={onRetry}
                        disabled={retrying()}
                      >
                        {retrying() ? "Retrying…" : "Retry"}
                      </button>
                    </Show>
                  </div>
                </Show>

                <ol class="step-list">
                  <For each={j().steps}>
                    {(step) => {
                      const canExpand = () => step.hasOutput;
                      const entry = () => expanded[step.name];
                      const isOpen = () => !!entry();
                      return (
                        <li class={`step step-${step.status}`}>
                          <button
                            type="button"
                            class="step-row"
                            onClick={() => {
                              if (canExpand()) void toggleExpand(step);
                            }}
                            disabled={!canExpand()}
                            aria-expanded={isOpen()}
                          >
                            <span class="step-icon" aria-hidden="true">
                              {stepIcon(step.status)}
                            </span>
                            <span class="step-name">{step.name}</span>
                            <Show when={step.durationMs != null}>
                              <span class="step-duration">
                                {formatDuration(step.durationMs)}
                              </span>
                            </Show>
                            <Show when={step.summary}>
                              <span class="step-summary">{step.summary}</span>
                            </Show>
                            <Show when={canExpand()}>
                              <span class="step-expand" aria-hidden="true">
                                {isOpen() ? "▾" : "▸"}
                              </span>
                            </Show>
                          </button>
                          <Show when={step.error}>
                            <div class="step-error">{step.error}</div>
                          </Show>
                          <Show when={entry()}>
                            {(e) => (
                              <div class="step-output">
                                <Switch>
                                  <Match when={e().state === "loading"}>
                                    <p class="loading">Loading output…</p>
                                  </Match>
                                  <Match
                                    when={(() => {
                                      const x = e();
                                      return x.state === "error" ? x : null;
                                    })()}
                                  >
                                    {(x) => <p class="error">{x().message}</p>}
                                  </Match>
                                  <Match
                                    when={(() => {
                                      const x = e();
                                      return x.state === "loaded" ? x : null;
                                    })()}
                                  >
                                    {(x) => (
                                      <pre>
                                        {JSON.stringify(x().output, null, 2)}
                                      </pre>
                                    )}
                                  </Match>
                                </Switch>
                              </div>
                            )}
                          </Show>
                        </li>
                      );
                    }}
                  </For>
                </ol>

                <Show when={j().recipeReady && j().recipeId}>
                  <div class="terminal-actions">
                    <button
                      type="button"
                      class="btn btn-primary view-recipe"
                      onClick={() =>
                        navigate(
                          j().status === ScrapeJobStatus.Normalizing
                            ? `/recipes/${j().recipeId}?scrapeJob=${j().id}`
                            : `/recipes/${j().recipeId}`,
                        )
                      }
                    >
                      View Recipe →
                    </button>
                  </div>
                </Show>
              </>
            );
          }}
        </Match>
      </Switch>
    </div>
  );
}
