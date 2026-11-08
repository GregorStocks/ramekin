import {
  createSignal,
  createMemo,
  createEffect,
  onCleanup,
  For,
  Show,
} from "solid-js";
import { A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import { extractApiError } from "../utils/recipeFormHelpers";
import { usePageTitle } from "../utils/pageTitle";
import { logger } from "../utils/logger";
import { reuseUnchanged } from "../utils/stableList";
import {
  loadShoppingListSyncCache,
  syncShoppingList,
  type ShoppingListSyncCache,
} from "../utils/shoppingListSyncCache";
import type { ShoppingListOp, SyncShoppingListItem } from "ramekin-client";

/** How long the toast offers to undo a delete. */
const UNDO_TOAST_MS = 8000;

/** A delete the toast offers to undo. */
interface Undoable {
  message: string;
  ids: string[];
}

export default function ShoppingListPage() {
  usePageTitle(() => "Shopping List");
  const { getSyncApi, token } = useAuth();

  const [items, setItems] = createSignal<SyncShoppingListItem[]>([]);
  const [categoryOrder, setCategoryOrder] = createSignal<string[]>([]);
  const [loading, setLoading] = createSignal(true);
  // Why the list couldn't load or refresh.
  const [loadError, setLoadError] = createSignal<string | null>(null);
  // Why the last user action failed. Each action clears it when it starts.
  const [actionError, setActionError] = createSignal<string | null>(null);
  const [undoable, setUndoable] = createSignal<Undoable | null>(null);
  const [newItemName, setNewItemName] = createSignal("");
  const [newItemAmount, setNewItemAmount] = createSignal("");
  const [adding, setAdding] = createSignal(false);
  const [clearingChecked, setClearingChecked] = createSignal(false);
  // Items whose category change is in flight; each row's select stays
  // disabled until its own change settles.
  const [updatingCategoryIds, setUpdatingCategoryIds] = createSignal<
    ReadonlySet<string>
  >(new Set());

  const hasCheckedItems = () => items().some((item) => item.isChecked);

  const groupedUncheckedItems = createMemo(() => {
    const unchecked = items()
      .filter((i) => !i.isChecked)
      .sort((a, b) => a.sortOrder - b.sortOrder);
    const grouped = new Map<string, SyncShoppingListItem[]>();
    for (const item of unchecked) {
      const cat = item.category;
      if (!grouped.has(cat)) grouped.set(cat, []);
      grouped.get(cat)!.push(item);
    }
    return grouped;
  });
  // Render groups keyed by category name so a category appearing or
  // emptying out doesn't rebuild the other groups' rows.
  const uncheckedCategories = createMemo(() =>
    categoryOrder().filter((cat) => groupedUncheckedItems().has(cat)),
  );

  const checkedItems = createMemo(() =>
    items()
      .filter((i) => i.isChecked)
      .sort((a, b) => a.sortOrder - b.sortOrder),
  );

  const applyCache = (cache: ShoppingListSyncCache) => {
    setItems((prev) => reuseUnchanged(prev, cache.items, (i) => i.id));
    setCategoryOrder(cache.categoryOrder);
  };

  const warnCache = (context: string, err: unknown) => {
    logger.warn("Shopping", `${context}: ${String(err)}`);
  };

  /** Write `ops` (or just refresh) and show the server's answer. */
  const sync = async (ops: ShoppingListOp[] = []) => {
    const requestToken = token();
    const cache = await logger.timed("Shopping", "syncShoppingList", () =>
      syncShoppingList(
        getSyncApi(),
        localStorage,
        requestToken,
        ops,
        warnCache,
      ),
    );
    if (requestToken !== token()) return;
    applyCache(cache);
    setLoadError(null);
  };

  /** Run a user action, showing why it failed in the one action-error slot. */
  const runAction = async (
    fallbackMessage: string,
    action: () => Promise<void>,
  ): Promise<boolean> => {
    setActionError(null);
    try {
      await action();
      return true;
    } catch (err) {
      setActionError(await extractApiError(err, fallbackMessage));
      return false;
    }
  };

  let undoTimer: ReturnType<typeof setTimeout> | undefined;
  onCleanup(() => clearTimeout(undoTimer));

  const offerUndo = (message: string, ids: string[]) => {
    clearTimeout(undoTimer);
    setUndoable({ message, ids });
    undoTimer = setTimeout(() => setUndoable(null), UNDO_TOAST_MS);
  };

  const dismissUndo = () => {
    clearTimeout(undoTimer);
    setUndoable(null);
  };

  const loadItems = async () => {
    const requestToken = token();
    let cached: ShoppingListSyncCache | null = null;
    try {
      cached = loadShoppingListSyncCache(localStorage, requestToken);
    } catch (err) {
      warnCache("shopping list cache read failed", err);
    }
    // A new session starts clean: nothing from the last one's list, undo
    // toast or errors carries over.
    dismissUndo();
    setActionError(null);
    setLoadError(null);
    if (cached) {
      applyCache(cached);
      setLoading(false);
    } else {
      setItems([]);
      setCategoryOrder([]);
      setLoading(true);
    }

    try {
      await sync();
    } catch (err) {
      const message = await extractApiError(
        err,
        cached
          ? "Failed to sync shopping list"
          : "Failed to load shopping list",
      );
      if (requestToken === token()) setLoadError(message);
    } finally {
      if (requestToken === token()) setLoading(false);
    }
  };

  createEffect(() => {
    loadItems();
  });

  const pageError = () => loadError() ?? actionError();

  const handleAddItem = async () => {
    const name = newItemName().trim();
    if (!name || adding()) return;

    setAdding(true);
    try {
      await runAction("Failed to add item", async () => {
        await sync([
          {
            op: "create",
            id: crypto.randomUUID(),
            fields: { item: name, amount: newItemAmount().trim() || undefined },
          },
        ]);
        setNewItemName("");
        setNewItemAmount("");
      });
    } finally {
      setAdding(false);
    }
  };

  /**
   * Send the state the click asked for, not the opposite of the row's last
   * synced state: a second click before the first sync returns then unchecks
   * rather than sending "checked" twice. Syncs run one at a time, in order.
   */
  const handleToggleChecked = async (
    item: SyncShoppingListItem,
    checkbox: HTMLInputElement,
  ) => {
    const isChecked = checkbox.checked;
    const ok = await runAction("Failed to update item", () =>
      sync([{ op: "update", id: item.id, fields: { isChecked } }]),
    );
    // The row didn't change, so Solid won't reset the box itself.
    if (!ok) checkbox.checked = item.isChecked;
  };

  const handleCategoryChange = async (
    item: SyncShoppingListItem,
    categoryOverride: string | null,
  ) => {
    setUpdatingCategoryIds((prev) => new Set(prev).add(item.id));
    try {
      // "" clears the override.
      await runAction("Failed to update category", () =>
        sync([
          {
            op: "update",
            id: item.id,
            fields: { categoryOverride: categoryOverride ?? "" },
          },
        ]),
      );
    } finally {
      setUpdatingCategoryIds((prev) => {
        const next = new Set(prev);
        next.delete(item.id);
        return next;
      });
    }
  };

  const handleDelete = (item: SyncShoppingListItem) =>
    runAction("Failed to delete item", async () => {
      await sync([{ op: "delete", id: item.id }]);
      offerUndo(`Deleted ${item.item}`, [item.id]);
    });

  // Deletes only the checked items on screen, so an item checked on another
  // device since the last sync is never removed unseen.
  const handleClearChecked = async () => {
    if (clearingChecked()) return;
    setClearingChecked(true);
    try {
      await runAction("Failed to clear checked items", async () => {
        const ids = items()
          .filter((i) => i.isChecked)
          .map((i) => i.id);
        await sync(ids.map((id): ShoppingListOp => ({ op: "delete", id })));
        offerUndo(
          ids.length === 1 ? "Cleared 1 item" : `Cleared ${ids.length} items`,
          ids,
        );
      });
    } finally {
      setClearingChecked(false);
    }
  };

  const handleUndo = (ids: string[]) =>
    runAction("Failed to undo", async () => {
      dismissUndo();
      await sync(ids.map((id): ShoppingListOp => ({ op: "restore", id })));
    });

  const renderItem = (item: SyncShoppingListItem) => (
    <li class="shopping-item" classList={{ checked: item.isChecked }}>
      <label class="shopping-checkbox-label">
        <input
          type="checkbox"
          checked={item.isChecked}
          onChange={(e) => handleToggleChecked(item, e.currentTarget)}
          class="shopping-checkbox"
        />
        <span class="shopping-item-content">
          <span class="shopping-item-name">{item.item}</span>
          <Show when={item.amount}>
            <span class="shopping-item-amount">{item.amount}</span>
          </Show>
        </span>
      </label>
      <Show when={item.sourceRecipeId && item.sourceRecipeTitle}>
        <A
          href={`/recipes/${item.sourceRecipeId}`}
          class="shopping-item-source"
        >
          {item.sourceRecipeTitle}
        </A>
      </Show>
      <select
        class="shopping-category-select"
        value={item.categoryOverride ?? ""}
        disabled={updatingCategoryIds().has(item.id)}
        title="Category"
        onChange={(e) =>
          handleCategoryChange(item, e.currentTarget.value || null)
        }
      >
        <option value="">Auto</option>
        <For each={categoryOrder()}>
          {(category) => <option value={category}>{category}</option>}
        </For>
      </select>
      <button
        class="shopping-item-delete"
        onClick={() => handleDelete(item)}
        title="Delete item"
      >
        &times;
      </button>
    </li>
  );

  return (
    <div class="shopping-list-page">
      <div class="page-header">
        <h2>Shopping List</h2>
        <div class="shopping-header-actions">
          <Show when={hasCheckedItems()}>
            <button
              class="btn btn-small"
              onClick={handleClearChecked}
              disabled={clearingChecked()}
            >
              {clearingChecked() ? "Clearing..." : "Clear Checked"}
            </button>
          </Show>
          <A href="/shopping-list/history" class="btn btn-small">
            History
          </A>
        </div>
      </div>

      <form
        class="shopping-add-form"
        onSubmit={(e) => {
          e.preventDefault();
          handleAddItem();
        }}
      >
        <input
          type="text"
          class="shopping-add-input"
          placeholder="Add an item..."
          value={newItemName()}
          onInput={(e) => setNewItemName(e.currentTarget.value)}
          disabled={adding()}
        />
        <input
          type="text"
          class="shopping-add-amount"
          placeholder="Amount"
          value={newItemAmount()}
          onInput={(e) => setNewItemAmount(e.currentTarget.value)}
          disabled={adding()}
        />
        <button
          type="submit"
          class="btn btn-primary btn-small"
          disabled={adding() || !newItemName().trim()}
        >
          {adding() ? "Adding..." : "Add"}
        </button>
      </form>

      <Show when={pageError()}>
        <div class="error-message">{pageError()}</div>
      </Show>

      <Show when={loading()}>
        <p class="loading-text">Loading shopping list...</p>
      </Show>

      <Show when={!loading() && items().length === 0}>
        <div class="empty-state">
          <p>Your shopping list is empty.</p>
          <p class="empty-hint">
            Add items above, or from recipes by clicking "Add to Shopping List"
            on any recipe page.
          </p>
        </div>
      </Show>

      <Show when={!loading() && items().length > 0}>
        <div class="shopping-list">
          <For each={uncheckedCategories()}>
            {(category) => (
              <div class="shopping-category-group">
                <h3 class="shopping-category-header">{category}</h3>
                <ul class="shopping-category-items">
                  <For each={groupedUncheckedItems().get(category) ?? []}>
                    {renderItem}
                  </For>
                </ul>
              </div>
            )}
          </For>

          <Show when={checkedItems().length > 0}>
            <div class="shopping-category-group checked-group">
              <h3 class="shopping-category-header">Checked</h3>
              <ul class="shopping-category-items">
                <For each={checkedItems()}>{renderItem}</For>
              </ul>
            </div>
          </Show>
        </div>
      </Show>

      <Show when={undoable()}>
        {(pending) => (
          <div class="shopping-undo-toast" role="status">
            <span>{pending().message}</span>
            <button
              class="shopping-undo-button"
              onClick={() => handleUndo(pending().ids)}
            >
              Undo
            </button>
          </div>
        )}
      </Show>
    </div>
  );
}
