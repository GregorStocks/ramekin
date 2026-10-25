import {
  createEffect,
  createSignal,
  For,
  Match,
  on,
  onCleanup,
  Show,
  Switch,
} from "solid-js";
import { A } from "@solidjs/router";
import type {
  NormalizeApi,
  NormalizeOpInfo,
  NormalizeProposalResponse,
  ScrapeApi,
  StepState,
} from "ramekin-client";
import Modal from "./Modal";
import NormalizeOpPicker from "./NormalizeOpPicker";
import RecipeContentDiff from "./RecipeContentDiff";
import {
  normalizedRecipeChanged,
  photosChanged,
} from "../utils/recipeContentChanges";
import {
  defaultNormalizeSelection,
  effectiveNormalizeOps,
  normalizeStepLabel,
  toggleNormalizeOp,
} from "../utils/normalizeSelection";
import {
  pollScrapeJob,
  PollScrapeJobAbortedError,
} from "../utils/pollScrapeJob";
import { extractApiError } from "../utils/recipeFormHelpers";

interface ReNormalizeModalProps {
  isOpen: () => boolean;
  onClose: () => void;
  recipeId: string;
  getNormalizeApi: () => NormalizeApi;
  getScrapeApi: () => ScrapeApi;
  /** The proposal was saved as the recipe's new version. */
  onAccepted: () => Promise<void>;
}

type Phase =
  | { kind: "loading" }
  | { kind: "choose" }
  | { kind: "running"; jobId: string; steps: StepState[] }
  | { kind: "review"; jobId: string; proposal: NormalizeProposalResponse }
  | { kind: "failed"; jobId: string; error: string };

const STEP_MARKS: Record<string, string> = {
  completed: "✓",
  running: "…",
  failed: "✗",
  skipped: "–",
};

/**
 * Re-normalize one recipe: pick ops, run them as a job, then review the
 * proposed version against the one it started from and accept or discard it.
 */
export default function ReNormalizeModal(props: ReNormalizeModalProps) {
  const [ops, setOps] = createSignal<NormalizeOpInfo[]>([]);
  const [selected, setSelected] = createSignal<Set<string>>(new Set());
  const [phase, setPhase] = createSignal<Phase>({ kind: "loading" });
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);
  let controller: AbortController | null = null;
  // Bumped whenever the modal opens or closes, so answers to a closed
  // session's requests are ignored.
  let session = 0;

  const abortRun = () => {
    controller?.abort();
    controller = null;
  };
  onCleanup(abortRun);

  createEffect(
    on(props.isOpen, (open) => {
      session += 1;
      abortRun();
      if (!open) return;
      const opened = session;
      setError(null);
      setPhase({ kind: "loading" });
      void props
        .getNormalizeApi()
        .listNormalizeOps({ recipeId: props.recipeId })
        .then((response) => {
          if (opened !== session) return;
          setOps(response.ops);
          setSelected(defaultNormalizeSelection(response.ops));
          setPhase({ kind: "choose" });
        })
        .catch(async (err: unknown) => {
          const message = await extractApiError(
            err,
            "Failed to load normalize ops",
          );
          if (opened !== session) return;
          setError(message);
          setPhase({ kind: "choose" });
        });
    }),
  );

  const chosenOps = () => effectiveNormalizeOps(ops(), selected());

  const start = async () => {
    setBusy(true);
    setError(null);
    abortRun();
    const run = new AbortController();
    controller = run;
    let jobId: string;
    try {
      const job = await props.getNormalizeApi().normalizeRecipe({
        id: props.recipeId,
        normalizeRequest: { ops: chosenOps() },
      });
      jobId = job.jobId;
    } catch (err) {
      const message = await extractApiError(err, "Failed to start normalizing");
      setBusy(false);
      // Closing the modal aborts the run; its answer belongs to no one.
      if (!run.signal.aborted) setError(message);
      return;
    }
    setBusy(false);
    if (run.signal.aborted) return;
    setPhase({ kind: "running", jobId, steps: [] });
    try {
      const result = await pollScrapeJob(props.getScrapeApi(), jobId, {
        signal: run.signal,
        // AI ops can be slow; closing the modal aborts the poll.
        timeoutMs: null,
        onUpdate: (job) =>
          setPhase({ kind: "running", jobId, steps: job.steps }),
      });
      if (result.status === "completed") {
        const proposal = await props
          .getNormalizeApi()
          .getNormalizeProposal({ id: props.recipeId, jobId });
        if (run.signal.aborted) return;
        setPhase({ kind: "review", jobId, proposal });
      } else if (result.status === "failed") {
        setPhase({ kind: "failed", jobId, error: result.error });
      }
    } catch (err) {
      if (err instanceof PollScrapeJobAbortedError || run.signal.aborted)
        return;
      setPhase({
        kind: "failed",
        jobId,
        error: await extractApiError(err, "Lost track of the normalize job"),
      });
    }
  };

  const accept = async (jobId: string) => {
    setBusy(true);
    setError(null);
    try {
      await props
        .getNormalizeApi()
        .acceptNormalizeProposal({ id: props.recipeId, jobId });
      await props.onAccepted();
      props.onClose();
    } catch (err) {
      setError(await extractApiError(err, "Failed to save the new version"));
    } finally {
      setBusy(false);
    }
  };

  const reviewChanges = (proposal: NormalizeProposalResponse) =>
    !!proposal.proposed &&
    normalizedRecipeChanged(proposal.base, proposal.proposed);

  const proposalPhotosChanged = (proposal: NormalizeProposalResponse) =>
    !!proposal.proposed && photosChanged(proposal.base, proposal.proposed);

  const actions = () => {
    const current = phase();
    switch (current.kind) {
      case "choose":
        return (
          <>
            <button type="button" class="btn" onClick={props.onClose}>
              Cancel
            </button>
            <button
              type="button"
              class="btn btn-primary"
              onClick={start}
              disabled={busy() || chosenOps().length === 0}
            >
              {busy() ? "Starting..." : "Run"}
            </button>
          </>
        );
      case "review":
        if (!reviewChanges(current.proposal)) {
          return (
            <button type="button" class="btn" onClick={props.onClose}>
              Close
            </button>
          );
        }
        return (
          <>
            <button type="button" class="btn" onClick={props.onClose}>
              Discard
            </button>
            <button
              type="button"
              class="btn btn-primary"
              onClick={() => accept(current.jobId)}
              disabled={busy()}
            >
              {busy() ? "Saving..." : "Accept"}
            </button>
          </>
        );
      default:
        return (
          <button type="button" class="btn" onClick={props.onClose}>
            Close
          </button>
        );
    }
  };

  return (
    <Modal
      isOpen={props.isOpen}
      onClose={props.onClose}
      title="Re-normalize recipe"
      actions={actions()}
    >
      <div class="renormalize">
        <Show when={error()}>
          <p class="error">{error()}</p>
        </Show>
        <Switch>
          <Match when={phase().kind === "loading"}>
            <p class="loading-text">Loading...</p>
          </Match>
          <Match when={phase().kind === "choose"}>
            <p class="renormalize-hint">
              Source ops replace the recipe's content, hand edits included. You
              review the result before it's saved.
            </p>
            <NormalizeOpPicker
              ops={ops()}
              selected={selected()}
              onToggle={(id) =>
                setSelected(toggleNormalizeOp(ops(), selected(), id))
              }
              disabled={busy()}
            />
          </Match>
          <Match
            when={(() => {
              const current = phase();
              return current.kind === "running" ? current : null;
            })()}
          >
            {(running) => (
              <ul class="renormalize-steps" aria-live="polite">
                <Show when={running().steps.length === 0}>
                  <li>Starting...</li>
                </Show>
                <For each={running().steps}>
                  {(step) => (
                    <li
                      class={`renormalize-step renormalize-step-${step.status}`}
                    >
                      <span class="renormalize-step-mark">
                        {STEP_MARKS[step.status] ?? "○"}
                      </span>
                      {normalizeStepLabel(step, ops())}
                    </li>
                  )}
                </For>
              </ul>
            )}
          </Match>
          <Match
            when={(() => {
              const current = phase();
              return current.kind === "review" ? current : null;
            })()}
          >
            {(review) => (
              <div class="enrich-preview">
                <Show when={proposalPhotosChanged(review().proposal)}>
                  <p class="renormalize-hint">The recipe's photos change.</p>
                </Show>
                <Show
                  when={review().proposal.proposed}
                  fallback={<p class="error">The job proposed nothing.</p>}
                >
                  {(proposed) => (
                    <RecipeContentDiff
                      oldContent={review().proposal.base.content}
                      newContent={proposed().content}
                      emptyMessage={
                        proposalPhotosChanged(review().proposal)
                          ? "No other fields change."
                          : "Nothing changes, so there is nothing to save."
                      }
                    />
                  )}
                </Show>
              </div>
            )}
          </Match>
          <Match
            when={(() => {
              const current = phase();
              return current.kind === "failed" ? current : null;
            })()}
          >
            {(failed) => (
              <p class="error">
                {failed().error}{" "}
                <A href={`/scrape/${failed().jobId}`}>See what happened</A>
              </p>
            )}
          </Match>
        </Switch>
      </div>
    </Modal>
  );
}
