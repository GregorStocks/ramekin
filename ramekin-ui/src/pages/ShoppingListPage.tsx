import { createSignal, createEffect, For, Show } from "solid-js";
import { A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import Modal from "../components/Modal";
import { extractApiError } from "../utils/recipeFormHelpers";
import type { ShoppingListItemResponse } from "ramekin-client";

export default function ShoppingListPage() {
  const { getShoppingListApi } = useAuth();

  const [items, setItems] = createSignal<ShoppingListItemResponse[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [deletingItem, setDeletingItem] =
    createSignal<ShoppingListItemResponse | null>(null);
  const [deleteLoading, setDeleteLoading] = createSignal(false);
  const [clearingChecked, setClearingChecked] = createSignal(false);

  const hasCheckedItems = () => items().some((item) => item.isChecked);

  const sortedItems = () => {
    const all = items();
    const unchecked = all
      .filter((i) => !i.isChecked)
      .sort((a, b) => a.sortOrder - b.sortOrder);
    const checked = all
      .filter((i) => i.isChecked)
      .sort((a, b) => a.sortOrder - b.sortOrder);
    return [...unchecked, ...checked];
  };

  const loadItems = async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await getShoppingListApi().listItems();
      setItems(response.items);
    } catch (err) {
      const message = await extractApiError(
        err,
        "Failed to load shopping list",
      );
      setError(message);
    } finally {
      setLoading(false);
    }
  };

  createEffect(() => {
    loadItems();
  });

  const handleToggleChecked = async (item: ShoppingListItemResponse) => {
    try {
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
    } catch (err) {
      const message = await extractApiError(err, "Failed to update item");
      setError(message);
    }
  };

  const confirmDelete = (item: ShoppingListItemResponse) => {
    setDeletingItem(item);
  };

  const handleDelete = async () => {
    const item = deletingItem();
    if (!item) return;

    setDeleteLoading(true);
    try {
      await getShoppingListApi().deleteItem({ id: item.id });
      setDeletingItem(null);
      setItems((prev) => prev.filter((i) => i.id !== item.id));
    } catch (err) {
      const message = await extractApiError(err, "Failed to delete item");
      setError(message);
      setDeletingItem(null);
    } finally {
      setDeleteLoading(false);
    }
  };

  const handleClearChecked = async () => {
    setClearingChecked(true);
    try {
      await getShoppingListApi().clearChecked();
      setItems((prev) => prev.filter((i) => !i.isChecked));
    } catch (err) {
      const message = await extractApiError(
        err,
        "Failed to clear checked items",
      );
      setError(message);
    } finally {
      setClearingChecked(false);
    }
  };

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

      <Show when={error()}>
        <div class="error-message">{error()}</div>
      </Show>

      <Show when={loading()}>
        <p class="loading-text">Loading shopping list...</p>
      </Show>

      <Show when={!loading() && items().length === 0}>
        <div class="empty-state">
          <p>Your shopping list is empty.</p>
          <p class="empty-hint">
            Add items from recipes by clicking "Add to Shopping List" on any
            recipe page.
          </p>
        </div>
      </Show>

      <Show when={!loading() && items().length > 0}>
        <ul class="shopping-list">
          <For each={sortedItems()}>
            {(item) => (
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
                <button
                  class="shopping-item-delete"
                  onClick={() => confirmDelete(item)}
                  title="Delete item"
                >
                  &times;
                </button>
              </li>
            )}
          </For>
        </ul>
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
