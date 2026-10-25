import { createEffect, createSignal, For, on, onCleanup, Show } from "solid-js";
import { A } from "@solidjs/router";
import type {
  NormalizeApi,
  NormalizeOpInfo,
  NormalizedRecipe,
  RecipeSummary,
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
  toggleNormalizeOp,
} from "../utils/normalizeSelection";
import { extractApiError } from "../utils/recipeFormHelpers";

interface BulkReNormalizeModalProps {
  isOpen: () => boolean;
  onClose: () => void;
  recipes: () => RecipeSummary[];
  getNormalizeApi: () => NormalizeApi;
  /** At least one proposal was saved. */
  onAccepted: () => Promise<void>;
}

type RowStatus =
  | "running"
  | "ready"
  | "unchanged"
  | "saving"
  | "accepted"
  | "discarded"
  | "failed";

interface Row {
  recipeId: string;
  title: string;
  jobId: string | null;
  status: RowStatus;
  error: string | null;
  base: NormalizedRecipe | null;
  proposed: NormalizedRecipe | null;
}

const POLL_INTERVAL_MS = 1500;

const STATUS_LABELS: Record<RowStatus, string> = {
  running: "Running…",
  ready: "Ready to review",
  unchanged: "No changes",
  saving: "Saving…",
  accepted: "Saved",
  discarded: "Discarded",
  failed: "Failed",
};

/**
 * Re-normalize a cookbook selection: one job per recipe, then each proposal
 * reviewed and accepted or discarded on its own, or all accepted at once.
 */
export default function BulkReNormalizeModal(props: BulkReNormalizeModalProps) {
  const [ops, setOps] = createSignal<NormalizeOpInfo[]>([]);
  const [selected, setSelected] = createSignal<Set<string>>(new Set());
  const [rows, setRows] = createSignal<Row[] | null>(null);
  const [expanded, setExpanded] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [starting, setStarting] = createSignal(false);
  let accepted = false;
  let polling: AbortController | null = null;

  const stopPolling = () => {
    polling?.abort();
    polling = null;
  };
  onCleanup(stopPolling);

  createEffect(
    on(props.isOpen, (open) => {
      stopPolling();
      if (!open) return;
      setRows(null);
      setExpanded(null);
      setError(null);
      accepted = false;
      void props
        .getNormalizeApi()
        .listNormalizeOps({})
        .then((response) => {
          setOps(response.ops);
          setSelected(defaultNormalizeSelection(response.ops));
        })
        .catch(async (err: unknown) =>
          setError(await extractApiError(err, "Failed to load normalize ops")),
        );
    }),
  );

  const updateRow = (recipeId: string, patch: Partial<Row>) =>
    setRows((current) =>
      current
        ? current.map((row) =>
            row.recipeId === recipeId ? { ...row, ...patch } : row,
          )
        : current,
    );

  const pollRows = async (signal: AbortSignal) => {
    while (!signal.aborted) {
      const running = (rows() ?? []).filter((row) => row.status === "running");
      if (running.length === 0) return;
      for (const row of running) {
        if (signal.aborted || !row.jobId) return;
        try {
          const proposal = await props.getNormalizeApi().getNormalizeProposal({
            id: row.recipeId,
            jobId: row.jobId,
          });
          if (signal.aborted) return;
          if (proposal.status === "completed") {
            const proposed = proposal.proposed ?? null;
            updateRow(row.recipeId, {
              status: !proposed
                ? "failed"
                : normalizedRecipeChanged(proposal.base, proposed)
                  ? "ready"
                  : "unchanged",
              error: proposed ? null : "The job proposed nothing",
              base: proposal.base,
              proposed,
            });
          } else if (proposal.status === "failed") {
            updateRow(row.recipeId, { status: "failed" });
          }
        } catch {
          // A missed poll is retried on the next pass.
        }
      }
      await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS));
    }
  };

  const start = async () => {
    const recipes = props.recipes();
    const titles = new Map(recipes.map((recipe) => [recipe.id, recipe.title]));
    setStarting(true);
    setError(null);
    try {
      const response = await props.getNormalizeApi().normalizeRecipes({
        bulkNormalizeRequest: {
          recipeIds: recipes.map((recipe) => recipe.id),
          ops: effectiveNormalizeOps(ops(), selected()),
        },
      });
      setRows(
        response.results.map((result) => ({
          recipeId: result.recipeId,
          title: titles.get(result.recipeId) ?? "Recipe",
          jobId: result.jobId ?? null,
          status: result.jobId ? "running" : "failed",
          error: result.error ?? null,
          base: null,
          proposed: null,
        })),
      );
    } catch (err) {
      setError(await extractApiError(err, "Failed to start normalizing"));
      return;
    } finally {
      setStarting(false);
    }
    stopPolling();
    const controller = new AbortController();
    polling = controller;
    void pollRows(controller.signal);
  };

  /** Save a recipe's proposal, if it is still waiting for a decision. */
  const accept = async (recipeId: string) => {
    // Rows are replaced on every update, so read the current one: the row may
    // have been discarded or saved since this was queued.
    const row = rows()?.find((candidate) => candidate.recipeId === recipeId);
    if (!row || row.status !== "ready" || !row.jobId) return;
    updateRow(row.recipeId, { status: "saving", error: null });
    try {
      await props.getNormalizeApi().acceptNormalizeProposal({
        id: row.recipeId,
        jobId: row.jobId,
      });
      accepted = true;
      updateRow(row.recipeId, { status: "accepted" });
      if (expanded() === row.recipeId) setExpanded(null);
    } catch (err) {
      updateRow(row.recipeId, {
        status: "ready",
        error: await extractApiError(err, "Failed to save the new version"),
      });
    }
  };

  const acceptAllReady = async () => {
    const ready = (rows() ?? []).filter((row) => row.status === "ready");
    for (const row of ready) await accept(row.recipeId);
  };

  const close = async () => {
    // A save in flight would finish after the cookbook refreshes; wait for it.
    if (counts().saving > 0) return;
    stopPolling();
    props.onClose();
    if (accepted) await props.onAccepted();
  };

  const counts = () => {
    const all = rows() ?? [];
    const count = (status: RowStatus) =>
      all.filter((row) => row.status === status).length;
    return {
      running: count("running"),
      ready: count("ready"),
      unchanged: count("unchanged"),
      accepted: count("accepted"),
      saving: count("saving"),
      failed: count("failed"),
    };
  };

  return (
    <Modal
      isOpen={props.isOpen}
      onClose={close}
      title={`Re-normalize ${props.recipes().length} recipes`}
      actions={
        <Show
          when={rows()}
          fallback={
            <>
              <button type="button" class="btn" onClick={close}>
                Cancel
              </button>
              <button
                type="button"
                class="btn btn-primary"
                onClick={start}
                disabled={
                  starting() ||
                  props.recipes().length === 0 ||
                  effectiveNormalizeOps(ops(), selected()).length === 0
                }
              >
                {starting() ? "Starting..." : "Run"}
              </button>
            </>
          }
        >
          <button
            type="button"
            class="btn"
            onClick={close}
            disabled={counts().saving > 0}
          >
            Close
          </button>
          <button
            type="button"
            class="btn btn-primary"
            onClick={acceptAllReady}
            disabled={counts().ready === 0}
          >
            Accept all ready ({counts().ready})
          </button>
        </Show>
      }
    >
      <div class="renormalize">
        <Show when={error()}>
          <p class="error">{error()}</p>
        </Show>
        <Show
          when={rows()}
          fallback={
            <>
              <p class="renormalize-hint">
                Each recipe gets its own run, and you review each result before
                it's saved. Source ops replace a recipe's content, hand edits
                included; recipes they can't run on are reported as failed.
              </p>
              <NormalizeOpPicker
                ops={ops()}
                selected={selected()}
                onToggle={(id) =>
                  setSelected(toggleNormalizeOp(ops(), selected(), id))
                }
                disabled={starting()}
              />
            </>
          }
        >
          {(currentRows) => (
            <>
              <p class="renormalize-hint" aria-live="polite">
                {counts().running} running · {counts().ready} to review ·{" "}
                {counts().accepted} saved · {counts().unchanged} unchanged ·{" "}
                {counts().failed} failed
              </p>
              <ul class="renormalize-rows">
                <For each={currentRows()}>
                  {(row) => (
                    <li class="renormalize-row">
                      <div class="renormalize-row-header">
                        <span class="renormalize-row-title">{row.title}</span>
                        <span class="renormalize-row-actions">
                          <span
                            class={`renormalize-row-status renormalize-row-${row.status}`}
                          >
                            {STATUS_LABELS[row.status]}
                          </span>
                          <Show when={row.status === "ready"}>
                            <button
                              type="button"
                              class="btn btn-small"
                              onClick={() =>
                                setExpanded(
                                  expanded() === row.recipeId
                                    ? null
                                    : row.recipeId,
                                )
                              }
                            >
                              {expanded() === row.recipeId ? "Hide" : "Review"}
                            </button>
                            <button
                              type="button"
                              class="btn btn-small"
                              onClick={() =>
                                updateRow(row.recipeId, { status: "discarded" })
                              }
                            >
                              Discard
                            </button>
                            <button
                              type="button"
                              class="btn btn-small btn-primary"
                              onClick={() => accept(row.recipeId)}
                            >
                              Accept
                            </button>
                          </Show>
                        </span>
                      </div>
                      <Show when={row.error}>
                        <p class="error">{row.error}</p>
                      </Show>
                      <Show when={row.status === "failed" && row.jobId}>
                        <A href={`/scrape/${row.jobId}`}>See what happened</A>
                      </Show>
                      <Show
                        when={
                          expanded() === row.recipeId &&
                          row.base &&
                          row.proposed
                        }
                      >
                        <Show when={photosChanged(row.base!, row.proposed!)}>
                          <p class="renormalize-hint">
                            The recipe's photos change.
                          </p>
                        </Show>
                        <RecipeContentDiff
                          oldContent={row.base!.content}
                          newContent={row.proposed!.content}
                          emptyMessage="No other fields change."
                        />
                      </Show>
                    </li>
                  )}
                </For>
              </ul>
            </>
          )}
        </Show>
      </div>
    </Modal>
  );
}
