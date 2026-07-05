import { createSignal, createMemo, createEffect, For, Show } from "solid-js";
import { A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import Modal from "../components/Modal";
import { extractApiError } from "../utils/recipeFormHelpers";
import { usePageTitle } from "../utils/pageTitle";
import { logger } from "../utils/logger";
import { createAsyncAction } from "../utils/asyncState";
import {
  applyShoppingListSyncResponse,
  loadShoppingListSyncCache,
  saveShoppingListSyncCache,
  type ShoppingListSyncCache,
} from "../utils/shoppingListSyncCache";
import type { ShoppingListItemResponse } from "ramekin-client";

export default function ShoppingListPage() {
  usePageTitle(() => "Shopping List");
  const { getShoppingListApi, token } = useAuth();

  const [items, setItems] = createSignal<ShoppingListItemResponse[]>([]);
  const [categoryOrder, setCategoryOrder] = createSignal<string[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [deletingItem, setDeletingItem] =
    createSignal<ShoppingListItemResponse | null>(null);
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
    const grouped = new Map<string, ShoppingListItemResponse[]>();
    for (const item of unchecked) {
      const cat = item.category;
      if (!grouped.has(cat)) grouped.set(cat, []);
      grouped.get(cat)!.push(item);
    }
    return categoryOrder()
      .filter((cat) => grouped.has(cat))
      .map((cat) => ({
        category: cat,
        items: grouped.get(cat)!,
      }));
  });

  const checkedItems = createMemo(() =>
    items()
      .filter((i) => i.isChecked)
      .sort((a, b) => a.sortOrder - b.sortOrder),
  );

  const applyCache = (cache: ShoppingListSyncCache) => {
    setItems(cache.items);
    setCategoryOrder(cache.categoryOrder);
  };

  const loadItems = async (showLoading = true) => {
    const cached = loadShoppingListSyncCache(localStorage, token());
    if (cached) {
      applyCache(cached);
      setLoading(false);
    } else if (showLoading) {
      setLoading(true);
    }

    setError(null);
    try {
      const response = await logger.timed("Shopping", "syncItems", () =>
        getShoppingListApi().syncItems({
          syncRequest: {
            lastSyncAt: cached?.lastSyncAt ?? undefined,
          },
        }),
      );
      const nextCache = applyShoppingListSyncResponse(cached, response);
      saveShoppingListSyncCache(localStorage, token(), nextCache);
      applyCache(nextCache);
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

    const amount = newItemAmount().trim() || undefined;
    await logger.timed("Shopping", "createItems", () =>
      getShoppingListApi().createItems({
        createShoppingListRequest: {
          items: [{ item: name, amount }],
        },
      }),
    );
    setNewItemName("");
    setNewItemAmount("");
    await loadItems(false);
  }, "Failed to add item");

  const toggleCheckedAction = createAsyncAction(
    async (item: ShoppingListItemResponse) => {
      await getShoppingListApi().updateItem({
        id: item.id,
        updateShoppingListItemRequest: {
          isChecked: !item.isChecked,
        },
      });
      setItems((prev) =>
        prev.map((i) =>
          i.id === item.id ? { ...i, isChecked: !i.isChecked } : i,
        ),
      );
    },
    "Failed to update item",
  );

  const updateCategoryAction = createAsyncAction(
    async (item: ShoppingListItemResponse, categoryOverride: string | null) => {
      setUpdatingCategoryId(item.id);
      try {
        await getShoppingListApi().updateItem({
          id: item.id,
          updateShoppingListItemRequest: {
            categoryOverride,
          },
        });
        await loadItems(false);
      } finally {
        setUpdatingCategoryId(null);
      }
    },
    "Failed to update category",
  );

  const deleteItemAction = createAsyncAction(
    async (item: ShoppingListItemResponse) => {
      await getShoppingListApi().deleteItem({ id: item.id });
      setDeletingItem(null);
      setItems((prev) => prev.filter((i) => i.id !== item.id));
    },
    "Failed to delete item",
    {
      onError: () => {
        setDeletingItem(null);
      },
    },
  );

  const clearCheckedAction = createAsyncAction(async () => {
    await getShoppingListApi().clearChecked();
    setItems((prev) => prev.filter((i) => !i.isChecked));
  }, "Failed to clear checked items");

  const adding = addItemAction.loading;
  const deleteLoading = deleteItemAction.loading;
  const clearingChecked = clearCheckedAction.loading;
  const pageError = () =>
    error() ??
    addItemAction.error() ??
    toggleCheckedAction.error() ??
    updateCategoryAction.error() ??
    deleteItemAction.error() ??
    clearCheckedAction.error();

  const clearActionErrors = () => {
    addItemAction.clearError();
    toggleCheckedAction.clearError();
    updateCategoryAction.clearError();
    deleteItemAction.clearError();
    clearCheckedAction.clearError();
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

  const handleToggleChecked = async (item: ShoppingListItemResponse) => {
    setError(null);
    clearActionErrors();
    await toggleCheckedAction.run(item);
  };

  const handleCategoryChange = async (
    item: ShoppingListItemResponse,
    categoryOverride: string | null,
  ) => {
    setError(null);
    clearActionErrors();
    await updateCategoryAction.run(item, categoryOverride);
  };

  const confirmDelete = (item: ShoppingListItemResponse) => {
    setDeletingItem(item);
  };

  const handleDelete = async () => {
    const item = deletingItem();
    if (!item) return;

    setError(null);
    clearActionErrors();
    await deleteItemAction.run(item);
  };

  const handleClearChecked = async () => {
    setError(null);
    clearActionErrors();
    await clearCheckedAction.run();
  };

  const renderItem = (item: ShoppingListItemResponse) => (
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
        onClick={() => confirmDelete(item)}
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
        <Show when={hasCheckedItems()}>
          <button
            class="btn btn-small"
            onClick={handleClearChecked}
            disabled={clearingChecked()}
          >
            {clearingChecked() ? "Clearing..." : "Clear Checked"}
          </button>
        </Show>
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
          <For each={groupedUncheckedItems()}>
            {(group) => (
              <div class="shopping-category-group">
                <h3 class="shopping-category-header">{group.category}</h3>
                <ul class="shopping-category-items">
                  <For each={group.items}>{renderItem}</For>
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

      <Modal
        isOpen={() => deletingItem() !== null}
        onClose={() => setDeletingItem(null)}
        title="Delete Item"
        actions={
          <>
            <button
              class="btn"
              onClick={() => setDeletingItem(null)}
              disabled={deleteLoading()}
            >
              Cancel
            </button>
            <button
              class="btn btn-danger"
              onClick={handleDelete}
              disabled={deleteLoading()}
            >
              {deleteLoading() ? "Deleting..." : "Delete"}
            </button>
          </>
        }
      >
        <p>Delete "{deletingItem()?.item}" from your shopping list?</p>
      </Modal>
    </div>
  );
}
