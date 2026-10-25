import { For, Show } from "solid-js";
import { A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import { usePageTitle } from "../utils/pageTitle";
import { createApiResource, createAsyncAction } from "../utils/asyncState";
import type { ShoppingListHistoryEntry } from "ramekin-client";
import "./ShoppingListPage.css";

export default function ShoppingListHistoryPage() {
  usePageTitle(() => "Shopping List History");
  const { getShoppingListApi } = useAuth();

  const history = createApiResource(
    () => getShoppingListApi().listShoppingListHistory(),
    "Failed to load shopping list history",
  );

  const undoAction = createAsyncAction(
    async (entry: ShoppingListHistoryEntry) => {
      await getShoppingListApi().undoShoppingListChange({ id: entry.id });
      await history.refetch();
    },
    "Failed to undo",
  );

  const formatTime = (date: Date) =>
    date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });

  return (
    <div class="shopping-list-page">
      <div class="page-header">
        <h2>Shopping List History</h2>
        <A href="/shopping-list" class="btn btn-small">
          Back to List
        </A>
      </div>

      <Show when={history.error() ?? undoAction.error()}>
        {(message) => <div class="error-message">{message()}</div>}
      </Show>

      <Show when={history.initialLoading()}>
        <p class="loading-text">Loading history...</p>
      </Show>

      <Show when={history.latest()?.entries.length === 0}>
        <div class="empty-state">
          <p>No changes yet.</p>
        </div>
      </Show>

      <ul class="shopping-history">
        <For each={history.latest()?.entries ?? []}>
          {(entry) => (
            <li
              class="shopping-history-entry"
              classList={{ undone: !!entry.undoneById }}
            >
              <div class="shopping-history-text">
                <span class="shopping-history-summary">{entry.summary}</span>
                <span class="shopping-history-time">
                  {formatTime(entry.createdAt)}
                </span>
              </div>
              <Show
                when={!entry.undoneById}
                fallback={<span class="shopping-history-undone">Undone</span>}
              >
                <button
                  class="btn btn-small"
                  onClick={() => undoAction.run(entry)}
                  disabled={undoAction.loading()}
                >
                  Undo
                </button>
              </Show>
            </li>
          )}
        </For>
      </ul>
    </div>
  );
}
