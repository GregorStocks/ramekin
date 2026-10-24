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
import { createAsyncAction } from "../utils/asyncState";
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
  const [error, setError] = createSignal<string | null>(null);
  const [undoable, setUndoable] = createSignal<Undoable | null>(null);
  const [newItemName, setNewItemName] = createSignal("");
  const [newItemAmount, setNewItemAmount] = createSignal("");
  const [updatingCategoryId, setUpdatingCategoryId] = createSignal<
    string | null
  >(null);

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
    if (requestToken === token()) applyCache(cache);
  };

  const loadItems = async () => {
    let cached: ShoppingListSyncCache | null = null;
    try {
      cached = loadShoppingListSyncCache(localStorage, token());
    } catch (err) {
      warnCache("shopping list cache read failed", err);
    }
    if (cached) {
      applyCache(cached);
      setLoading(false);
    }

    setError(null);
    try {
      await sync();
    } catch (err) {
      const message = await extractApiError(
        err,
        cached
          ? "Failed to sync shopping list"
          : "Failed to load shopping list",
      );
      setError(message);
    } finally {
      setLoading(false);
    }
  };

  const addItemAction = createAsyncAction(async () => {
    const name = newItemName().trim();
    if (!name) return;

    await sync([
      {
        op: "create",
        id: crypto.randomUUID(),
        fields: { item: name, amount: newItemAmount().trim() || undefined },
      },
    ]);
    setNewItemName("");
    setNewItemAmount("");
  }, "Failed to add item");

  const toggleCheckedAction = createAsyncAction(
    async (item: SyncShoppingListItem) => {
      await sync([
        { op: "update", id: item.id, fields: { isChecked: !item.isChecked } },
      ]);
    },
    "Failed to update item",
  );

  const updateCategoryAction = createAsyncAction(
    async (item: SyncShoppingListItem, categoryOverride: string | null) => {
      setUpdatingCategoryId(item.id);
      try {
        // "" clears the override.
        await sync([
          {
            op: "update",
            id: item.id,
            fields: { categoryOverride: categoryOverride ?? "" },
          },
        ]);
      } finally {
        setUpdatingCategoryId(null);
      }
    },
    "Failed to update category",
  );

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

  const deleteItemAction = createAsyncAction(
    async (item: SyncShoppingListItem) => {
      await sync([{ op: "delete", id: item.id }]);
      offerUndo(`Deleted ${item.item}`, [item.id]);
    },
    "Failed to delete item",
  );

  // Deletes only the checked items on screen, so an item checked on another
  // device since the last sync is never removed unseen.
  const clearCheckedAction = createAsyncAction(async () => {
    const ids = items()
      .filter((i) => i.isChecked)
      .map((i) => i.id);
    await sync(ids.map((id): ShoppingListOp => ({ op: "delete", id })));
    offerUndo(
      ids.length === 1 ? "Cleared 1 item" : `Cleared ${ids.length} items`,
      ids,
    );
  }, "Failed to clear checked items");

  const undoDeleteAction = createAsyncAction(async (ids: string[]) => {
    dismissUndo();
    await sync(ids.map((id): ShoppingListOp => ({ op: "restore", id })));
  }, "Failed to undo");

  const adding = addItemAction.loading;
  const clearingChecked = clearCheckedAction.loading;
  const pageError = () =>
    error() ??
    addItemAction.error() ??
    toggleCheckedAction.error() ??
    updateCategoryAction.error() ??
    deleteItemAction.error() ??
    clearCheckedAction.error() ??
    undoDeleteAction.error();

  const clearActionErrors = () => {
    addItemAction.clearError();
    toggleCheckedAction.clearError();
    updateCategoryAction.clearError();
    deleteItemAction.clearError();
    clearCheckedAction.clearError();
    undoDeleteAction.clearError();
  };

  createEffect(() => {
    loadItems();
  });

  const handleAddItem = async () => {
    const name = newItemName().trim();
    if (!name) return;

    setError(null);
    clearActionErrors();
    await addItemAction.run();
  };

  const handleToggleChecked = async (item: SyncShoppingListItem) => {
    setError(null);
    clearActionErrors();
    await toggleCheckedAction.run(item);
  };

  const handleCategoryChange = async (
    item: SyncShoppingListItem,
    categoryOverride: string | null,
  ) => {
    setError(null);
    clearActionErrors();
    await updateCategoryAction.run(item, categoryOverride);
  };

  const handleDelete = async (item: SyncShoppingListItem) => {
    setError(null);
    clearActionErrors();
    await deleteItemAction.run(item);
  };

  const handleClearChecked = async () => {
    setError(null);
    clearActionErrors();
    await clearCheckedAction.run();
  };

  const handleUndo = async (ids: string[]) => {
    setError(null);
    clearActionErrors();
    await undoDeleteAction.run(ids);
  };

  const renderItem = (item: SyncShoppingListItem) => (
    <li class="shopping-item" classList={{ checked: item.isChecked }}>
      <label class="shopping-checkbox-label">
        <input
          type="checkbox"
          checked={item.isChecked}
          onChange={() => handleToggleChecked(item)}
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
        disabled={updatingCategoryId() === item.id}
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
