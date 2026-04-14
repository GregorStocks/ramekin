import { createSignal, createMemo, createEffect, For, Show } from "solid-js";
import { A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import Modal from "../components/Modal";
import { extractApiError } from "../utils/recipeFormHelpers";
import { usePageTitle } from "../utils/pageTitle";
import type { ShoppingListItemResponse } from "ramekin-client";

const CATEGORY_ORDER = [
  "Produce",
  "Meat & Seafood",
  "Dairy & Eggs",
  "Cheese",
  "Bakery & Bread",
  "Frozen",
  "Pasta & Rice",
  "Canned Goods",
  "Baking",
  "Spices & Seasonings",
  "Condiments & Sauces",
  "Oils & Vinegars",
  "Nuts & Dried Fruit",
  "Beverages",
  "Snacks",
  "Other",
];

export default function ShoppingListPage() {
  usePageTitle(() => "Shopping List");
  const { getShoppingListApi } = useAuth();

  const [items, setItems] = createSignal<ShoppingListItemResponse[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [deletingItem, setDeletingItem] =
    createSignal<ShoppingListItemResponse | null>(null);
  const [deleteLoading, setDeleteLoading] = createSignal(false);
  const [clearingChecked, setClearingChecked] = createSignal(false);
  const [newItemName, setNewItemName] = createSignal("");
  const [newItemAmount, setNewItemAmount] = createSignal("");
  const [adding, setAdding] = createSignal(false);

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
    return CATEGORY_ORDER.filter((cat) => grouped.has(cat)).map((cat) => ({
      category: cat,
      items: grouped.get(cat)!,
    }));
  });

  const checkedItems = createMemo(() =>
    items()
      .filter((i) => i.isChecked)
      .sort((a, b) => a.sortOrder - b.sortOrder),
  );

  const loadItems = async (showLoading = true) => {
    if (showLoading) setLoading(true);
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

  const handleAddItem = async () => {
    const name = newItemName().trim();
    if (!name) return;

    setAdding(true);
    setError(null);
    try {
      const amount = newItemAmount().trim() || undefined;
      await getShoppingListApi().createItems({
        createShoppingListRequest: {
          items: [{ item: name, amount }],
        },
      });
      setNewItemName("");
      setNewItemAmount("");
      await loadItems(false);
    } catch (err) {
      const message = await extractApiError(err, "Failed to add item");
      setError(message);
    } finally {
      setAdding(false);
    }
  };

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
