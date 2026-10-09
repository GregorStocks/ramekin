import { createSignal, createEffect, For, onCleanup, Show } from "solid-js";
import { useAuth } from "../context/AuthContext";
import Modal from "./Modal";
import { extractApiError } from "../utils/recipeFormHelpers";
import {
  formatIngredient,
  formatIngredientAmount,
} from "../utils/ingredientFormatting";
import { logger } from "../utils/logger";
import {
  clearShoppingListSyncCache,
  refreshShoppingListSyncCache,
} from "../utils/shoppingListSyncCache";
import type { RecipeResponse } from "ramekin-client";

interface AddToShoppingListModalProps {
  isOpen: () => boolean;
  onClose: () => void;
  recipe: RecipeResponse;
  scale?: () => number;
}

export default function AddToShoppingListModal(
  props: AddToShoppingListModalProps,
) {
  const { getShoppingListApi, token } = useAuth();

  const [selectedIndices, setSelectedIndices] = createSignal<Set<number>>(
    new Set(),
  );
  const [adding, setAdding] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [showSuccess, setShowSuccess] = createSignal(false);
  const [wasOpen, setWasOpen] = createSignal(false);
  // Bumped on every open and close so a submission or close timer from an
  // earlier presentation can't touch the one that is showing now.
  let presentation = 0;
  let closeTimer: ReturnType<typeof setTimeout> | null = null;

  const clearCloseTimer = () => {
    if (closeTimer) {
      clearTimeout(closeTimer);
      closeTimer = null;
    }
  };

  const close = () => {
    presentation++;
    clearCloseTimer();
    props.onClose();
  };

  const scale = () => props.scale?.() ?? 1;
  const ingredients = () => props.recipe.ingredients ?? [];

  // Select all ingredients by default when modal opens
  createEffect(() => {
    const open = props.isOpen();
    if (open && !wasOpen()) {
      presentation++;
      clearCloseTimer();
      const allIndices = new Set(ingredients().map((_, i) => i));
      setSelectedIndices(allIndices);
      setAdding(false);
      setError(null);
      setShowSuccess(false);
    }
    setWasOpen(open);
  });

  onCleanup(() => {
    clearCloseTimer();
  });

  const allSelected = () => selectedIndices().size === ingredients().length;

  const toggleIngredient = (index: number) => {
    setSelectedIndices((prev) => {
      const next = new Set(prev);
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      return next;
    });
  };

  const toggleAll = () => {
    if (allSelected()) {
      setSelectedIndices(new Set<number>());
    } else {
      setSelectedIndices(new Set(ingredients().map((_, i) => i)));
    }
  };

  const handleAdd = async () => {
    const selected = selectedIndices();
    if (selected.size === 0) return;

    const current = presentation;
    const isCurrent = () => presentation === current;
    setAdding(true);
    setError(null);
    try {
      const items = ingredients()
        .filter((_, i) => selected.has(i))
        .map((ing) => ({
          item: ing.item,
          amount: formatIngredientAmount(ing, scale()),
          sourceRecipeId: props.recipe.id,
          sourceRecipeTitle: props.recipe.title,
        }));

      const api = getShoppingListApi();
      await api.createItems({
        createShoppingListRequest: { items },
      });
      try {
        await refreshShoppingListSyncCache(api, localStorage, token());
      } catch (refreshErr) {
        clearShoppingListSyncCache(localStorage, token());
        logger.warn(
          "Shopping",
          `cache refresh after recipe add failed: ${String(refreshErr)}`,
        );
      }

      if (!isCurrent()) return;
      setShowSuccess(true);
      clearCloseTimer();
      closeTimer = setTimeout(close, 1500);
    } catch (err) {
      const message = await extractApiError(
        err,
        "Failed to add items to shopping list",
      );
      if (isCurrent()) setError(message);
    } finally {
      if (isCurrent()) setAdding(false);
    }
  };

  const selectedCount = () => selectedIndices().size;

  return (
    <Modal
      isOpen={props.isOpen}
      onClose={close}
      title="Add to Shopping List"
      actions={
        <Show when={!showSuccess()}>
          <button class="btn" onClick={close} disabled={adding()}>
            Cancel
          </button>
          <button
            class="btn btn-primary"
            onClick={handleAdd}
            disabled={adding() || selectedCount() === 0}
          >
            {adding()
              ? "Adding..."
              : `Add ${selectedCount()} item${selectedCount() !== 1 ? "s" : ""}`}
          </button>
        </Show>
      }
    >
      <div class="add-shopping-modal">
        <Show when={showSuccess()}>
          <div class="add-shopping-success">
            Added {selectedCount()} item{selectedCount() !== 1 ? "s" : ""} to
            shopping list!
          </div>
        </Show>

        <Show when={!showSuccess()}>
          <Show when={error()}>
            <div class="error-message" style={{ "margin-bottom": "1rem" }}>
              {error()}
            </div>
          </Show>

          <div class="ingredient-select-header">
            <span>Select ingredients to add</span>
            <button type="button" class="select-all-btn" onClick={toggleAll}>
              {allSelected() ? "Deselect All" : "Select All"}
            </button>
          </div>

          <div class="ingredient-select-list">
            <For each={ingredients()}>
              {(ing, index) => (
                <label class="ingredient-select-item">
                  <input
                    type="checkbox"
                    checked={selectedIndices().has(index())}
                    onChange={() => toggleIngredient(index())}
                  />
                  <span class="ingredient-text">
                    {formatIngredient(ing, {
                      scale: scale(),
                      includeNote: true,
                    })}
                  </span>
                </label>
              )}
            </For>
          </div>
        </Show>
      </div>
    </Modal>
  );
}
