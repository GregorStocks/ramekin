import {
  createSignal,
  createMemo,
  onCleanup,
  onMount,
  Show,
  For,
} from "solid-js";
import { createStore, produce } from "solid-js/store";
import { useNavigate, useParams, A } from "@solidjs/router";
import type { ScrapeJobResponse, StepState } from "ramekin-client";

import { useAuth } from "../context/AuthContext";
import { usePageTitle } from "../utils/pageTitle";
import { extractApiError } from "../utils/recipeFormHelpers";
import {
  isTerminalScrapeJobStatus,
  PollScrapeJobAbortedError,
  pollScrapeJob,
} from "../utils/pollScrapeJob";

interface ExpandedOutput {
  loading: boolean;
  error?: string;
  output?: unknown;
}

function formatDuration(ms: number | null | undefined): string {
  if (ms == null) return "";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function stepIcon(status: string): string {
  switch (status) {
    case "completed":
      return "\u2713"; // ✓
    case "failed":
      return "\u2717"; // ✗
    case "running":
      return "\u25D0"; // ◐
    case "skipped":
      return "\u2212"; // −
    default:
      return "\u25CB"; // ○
  }
}

function statusLabel(status: string): string {
  switch (status) {
    case "pending":
      return "Pending";
    case "scraping":
      return "Scraping";
    case "parsing":
      return "Parsing";
    case "completed":
      return "Completed";
    case "failed":
      return "Failed";
    default:
      return status;
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

  let tickInterval: ReturnType<typeof setInterval> | null = null;
  let pollController: AbortController | null = null;

  const startPolling = () => {
    pollController?.abort();
    const id = params.id;
    if (!id) return;
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
        setPollError(message);
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
        setPollError(message);
      });
  };

  onMount(() => {
    startPolling();
    tickInterval = setInterval(() => setNow(Date.now()), 1000);
  });

  onCleanup(() => {
    pollController?.abort();
    pollController = null;
    if (tickInterval) {
      clearInterval(tickInterval);
      tickInterval = null;
    }
  });

  const toggleExpand = async (step: StepState) => {
    const id = params.id;
    if (!id || !step.hasOutput) return;
    const name = step.name;
    const existing = expanded[name];
    if (existing) {
      setExpanded(
        produce((s) => {
          delete s[name];
        }),
      );
      return;
    }
    setExpanded(name, { loading: true });
    try {
      const output = await getScrapeApi().getStepOutput({
        id,
        stepName: name,
      });
      setExpanded(name, { loading: false, output });
    } catch (err) {
      const message = await extractApiError(err, "Failed to load step output");
      setExpanded(name, { loading: false, error: message });
    }
  };

  const onRetry = async () => {
    const id = params.id;
    if (!id) return;
    setRetrying(true);
    try {
      await getScrapeApi().retryScrape({ id });
      // Clear any expanded outputs since we're starting over.
      setExpanded(
        produce((s) => {
          for (const key of Object.keys(s)) {
            delete s[key];
          }
        }),
      );
      startPolling();
    } catch (err) {
      const message = await extractApiError(err, "Failed to retry scrape");
      setPollError(message);
    } finally {
      setRetrying(false);
    }
  };

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

  const isTerminal = createMemo(() => {
    const j = job();
    return j ? isTerminalScrapeJobStatus(j.status) : false;
  });

  return (
    <div class="scrape-status-page">
      <Show when={pollError() && !job()}>
        <div class="error-state">
          <p class="error">{pollError()}</p>
          <A href="/" class="btn">
            Back to Cookbook
          </A>
        </div>
      </Show>

      <Show when={!pollError() && !job()}>
        <p class="loading">Loading scrape…</p>
      </Show>

      <Show when={job()}>
        {(j) => (
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

            <Show when={pollError() && job()}>
              <div class="scrape-poll-error">{pollError()}</div>
            </Show>

            <Show when={j().status === "failed" && j().error}>
              <div class="error-banner">
                <div>
                  <strong>
                    Failed
                    <Show when={j().failedAtStep}> at {j().failedAtStep}</Show>:
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
                            <Show when={e().loading}>
                              <p class="loading">Loading output…</p>
                            </Show>
                            <Show when={e().error}>
                              <p class="error">{e().error}</p>
                            </Show>
                            <Show
                              when={!e().loading && e().output !== undefined}
                            >
                              <pre>{JSON.stringify(e().output, null, 2)}</pre>
                            </Show>
                          </div>
                        )}
                      </Show>
                    </li>
                  );
                }}
              </For>
            </ol>

            <Show
              when={isTerminal() && j().status === "completed" && j().recipeId}
            >
              <div class="terminal-actions">
                <button
                  type="button"
                  class="btn btn-primary view-recipe"
                  onClick={() => navigate(`/recipes/${j().recipeId}`)}
                >
                  View Recipe →
                </button>
              </div>
            </Show>
          </>
        )}
      </Show>
    </div>
  );
}
