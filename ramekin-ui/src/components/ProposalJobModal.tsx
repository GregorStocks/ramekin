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
import type { JSX } from "solid-js";
import { A } from "@solidjs/router";
import type {
  EditRecipeApi,
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

/**
 * What the job does: normalize ops the user picks, or an Edit with AI run on
 * the user's instruction, with the recipe's photos as context.
 */
export type ProposalJobMode =
  | { kind: "normalize" }
  | {
      kind: "edit";
      getEditRecipeApi: () => EditRecipeApi;
      photoIds: string[];
    };

interface ProposalJobModalProps {
  isOpen: () => boolean;
  onClose: () => void;
  recipeId: string;
  mode: ProposalJobMode;
  getNormalizeApi: () => NormalizeApi;
  getScrapeApi: () => ScrapeApi;
  /** The proposal was saved as the recipe's new version. */
  onAccepted: () => Promise<void>;
}

/**
 * Each phase carries its own data and errors, so nothing from one phase (a
 * proposal, a failed save) can show in another.
 */
type Phase =
  | { kind: "loading" }
  /** The normalize ops didn't load; Retry loads them again. */
  | { kind: "loadFailed"; error: string }
  /** `error` says why the last Run didn't start. */
  | { kind: "choose"; error: string | null }
  | { kind: "starting" }
  | { kind: "running"; jobId: string; steps: StepState[] }
  /** `error` says why the last Accept didn't save. */
  | {
      kind: "review";
      jobId: string;
      proposal: NormalizeProposalResponse;
      accepting: boolean;
      error: string | null;
    }
  /** The job failed or was lost; Try again goes back to choosing. */
  | { kind: "failed"; jobId: string; error: string };

const STEP_MARKS: Record<string, string> = {
  completed: "✓",
  running: "…",
  failed: "✗",
  skipped: "–",
};

/**
 * Re-normalize or Edit with AI on one recipe: pick ops (or describe the
 * change), run them as a job, then review the proposed version against the
 * one it started from and accept or discard it.
 */
export default function ProposalJobModal(props: ProposalJobModalProps) {
  const [ops, setOps] = createSignal<NormalizeOpInfo[]>([]);
  const [instruction, setInstruction] = createSignal("");
  const [selected, setSelected] = createSignal<Set<string>>(new Set());
  const [phase, setPhase] = createSignal<Phase>({ kind: "loading" });
  let controller: AbortController | null = null;
  // Bumped whenever the modal opens or closes, so answers to a closed
  // session's requests are ignored.
  let session = 0;

  const abortRun = () => {
    controller?.abort();
    controller = null;
  };
  onCleanup(abortRun);

  const loadOps = () => {
    const opened = session;
    setPhase({ kind: "loading" });
    void props
      .getNormalizeApi()
      .listNormalizeOps({ recipeId: props.recipeId })
      .then((response) => {
        if (opened !== session) return;
        setOps(response.ops);
        setSelected(defaultNormalizeSelection(response.ops));
        setPhase({ kind: "choose", error: null });
      })
      .catch(async (err: unknown) => {
        const message = await extractApiError(
          err,
          "Failed to load normalize ops",
        );
        if (opened !== session) return;
        setPhase({ kind: "loadFailed", error: message });
      });
  };

  createEffect(
    on(props.isOpen, (open) => {
      session += 1;
      abortRun();
      if (!open) return;
      setOps([]);
      setSelected(new Set<string>());
      setInstruction("");
      if (props.mode.kind === "edit") {
        setPhase({ kind: "choose", error: null });
        return;
      }
      loadOps();
    }),
  );

  const chosenOps = () => effectiveNormalizeOps(ops(), selected());
  const canRun = () =>
    phase().kind === "choose" &&
    (props.mode.kind === "edit"
      ? instruction().trim().length > 0
      : chosenOps().length > 0);
  /** Starting a job or saving its proposal: the choices are locked. */
  const busy = () => {
    const current = phase();
    return (
      current.kind === "starting" ||
      (current.kind === "review" && current.accepting)
    );
  };
  /** Closing mid-save would leave the save's outcome unseen. */
  const close = () => {
    if (!busy()) props.onClose();
  };

  const startJob = async (): Promise<string> => {
    const mode = props.mode;
    if (mode.kind === "edit") {
      const job = await mode.getEditRecipeApi().editRecipeWithAi({
        id: props.recipeId,
        editRecipeRequest: {
          instruction: instruction().trim(),
          photoIds: mode.photoIds,
        },
      });
      return job.jobId;
    }
    const job = await props.getNormalizeApi().normalizeRecipe({
      id: props.recipeId,
      normalizeRequest: { ops: chosenOps() },
    });
    return job.jobId;
  };

  const start = async () => {
    if (!canRun()) return;
    setPhase({ kind: "starting" });
    abortRun();
    const run = new AbortController();
    controller = run;
    let jobId: string;
    try {
      jobId = await startJob();
    } catch (err) {
      const message = await extractApiError(err, "Failed to start");
      // Closing the modal aborts the run; its answer belongs to no one.
      if (!run.signal.aborted) setPhase({ kind: "choose", error: message });
      return;
    }
    if (run.signal.aborted) return;
    setPhase({ kind: "running", jobId, steps: [] });
    try {
      const result = await pollScrapeJob(props.getScrapeApi(), jobId, {
        signal: run.signal,
        // AI can be slow; closing the modal aborts the poll.
        timeoutMs: null,
        onUpdate: (job) =>
          setPhase({ kind: "running", jobId, steps: job.steps }),
      });
      if (result.status === "completed") {
        const proposal = await props
          .getNormalizeApi()
          .getNormalizeProposal({ id: props.recipeId, jobId });
        if (run.signal.aborted) return;
        setPhase({
          kind: "review",
          jobId,
          proposal,
          accepting: false,
          error: null,
        });
      } else if (result.status === "failed") {
        setPhase({ kind: "failed", jobId, error: result.error });
      }
    } catch (err) {
      if (err instanceof PollScrapeJobAbortedError || run.signal.aborted)
        return;
      setPhase({
        kind: "failed",
        jobId,
        error: await extractApiError(err, "Lost track of the job"),
      });
    }
  };

  const accept = async () => {
    const current = phase();
    if (current.kind !== "review" || current.accepting) return;
    const acceptSession = session;
    setPhase({ ...current, accepting: true, error: null });
    try {
      await props
        .getNormalizeApi()
        .acceptNormalizeProposal({ id: props.recipeId, jobId: current.jobId });
    } catch (err) {
      const message = await extractApiError(
        err,
        "Failed to save the new version",
      );
      // A save from a session that has since closed answers to no one.
      if (acceptSession !== session) return;
      setPhase({ ...current, accepting: false, error: message });
      return;
    }
    // The save landed, so the recipe changed even if this session closed.
    await props.onAccepted();
    if (acceptSession === session) props.onClose();
  };

  const tryAgain = () => {
    if (phase().kind === "failed") setPhase({ kind: "choose", error: null });
  };

  const reviewChanges = (proposal: NormalizeProposalResponse) =>
    !!proposal.proposed &&
    normalizedRecipeChanged(proposal.base, proposal.proposed);

  const proposalPhotosChanged = (proposal: NormalizeProposalResponse) =>
    !!proposal.proposed && photosChanged(proposal.base, proposal.proposed);

  const actions = (): JSX.Element => {
    const current = phase();
    switch (current.kind) {
      case "choose":
      case "starting":
        return (
          <>
            <button type="button" class="btn" onClick={close} disabled={busy()}>
              Cancel
            </button>
            <button
              type="button"
              class="btn btn-primary"
              onClick={start}
              disabled={!canRun()}
            >
              {current.kind === "starting" ? "Starting..." : "Run"}
            </button>
          </>
        );
      case "loading":
      case "loadFailed":
        return (
          <button type="button" class="btn" onClick={close}>
            Cancel
          </button>
        );
      case "review":
        if (!reviewChanges(current.proposal)) {
          return (
            <button type="button" class="btn" onClick={close}>
              Close
            </button>
          );
        }
        return (
          <>
            <button
              type="button"
              class="btn"
              onClick={close}
              disabled={current.accepting}
            >
              Discard
            </button>
            <button
              type="button"
              class="btn btn-primary"
              onClick={accept}
              disabled={current.accepting}
            >
              {current.accepting ? "Saving..." : "Accept"}
            </button>
          </>
        );
      default:
        return (
          <button type="button" class="btn" onClick={close}>
            Close
          </button>
        );
    }
  };

  const phaseError = () => {
    const current = phase();
    return current.kind === "choose" || current.kind === "review"
      ? current.error
      : null;
  };
  const choosing = () =>
    phase().kind === "choose" || phase().kind === "starting";

  return (
    <Modal
      isOpen={props.isOpen}
      onClose={close}
      title={
        props.mode.kind === "edit" ? "Edit with AI" : "Re-normalize recipe"
      }
      actions={actions()}
    >
      <div class="renormalize">
        <Show when={phaseError()}>
          <p class="error">{phaseError()}</p>
        </Show>
        <Switch>
          <Match when={phase().kind === "loading"}>
            <p class="loading-text">Loading...</p>
          </Match>
          <Match
            when={(() => {
              const current = phase();
              return current.kind === "loadFailed" ? current : null;
            })()}
          >
            {(failed) => (
              <div class="error">
                <p>{failed().error}</p>
                <button type="button" class="btn btn-small" onClick={loadOps}>
                  Retry
                </button>
              </div>
            )}
          </Match>
          <Match when={choosing() && props.mode.kind === "edit"}>
            <label class="edit-with-ai-label" for="edit-with-ai-instruction">
              What would you like to change?
            </label>
            <textarea
              id="edit-with-ai-instruction"
              class="edit-with-ai-instruction"
              rows={3}
              placeholder="e.g., make this vegan, convert to grams, cut it to 500 kcal a serving..."
              value={instruction()}
              onInput={(e) => setInstruction(e.currentTarget.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey && canRun() && !busy()) {
                  e.preventDefault();
                  void start();
                }
              }}
              disabled={busy()}
            />
            <p class="renormalize-hint">
              The AI looks up ingredient weights and calories as it works. You
              review the result before it's saved.
            </p>
          </Match>
          <Match when={choosing()}>
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
              <div class="proposal-review">
                <Show when={review().proposal.summary}>
                  {(summary) => <p class="proposal-summary">{summary()}</p>}
                </Show>
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
              <div class="error">
                <p>
                  {failed().error}{" "}
                  <A href={`/scrape/${failed().jobId}`}>See what happened</A>
                </p>
                <button type="button" class="btn btn-small" onClick={tryAgain}>
                  Try again
                </button>
              </div>
            )}
          </Match>
        </Switch>
      </div>
    </Modal>
  );
}
