import { createSignal, createEffect, For, Show } from "solid-js";
import { useAuth } from "../context/AuthContext";
import Modal from "./Modal";
import { extractApiError } from "../utils/recipeFormHelpers";
import type { RecipeResponse, Ingredient } from "ramekin-client";

interface AddToShoppingListModalProps {
  isOpen: () => boolean;
  onClose: () => void;
  recipe: RecipeResponse;
}

function formatIngredient(ing: Ingredient): string {
  const parts: string[] = [];
  if (ing.measurements[0]?.amount) {
    parts.push(ing.measurements[0].amount);
  }
  if (ing.measurements[0]?.unit) {
    parts.push(ing.measurements[0].unit);
  }
  parts.push(ing.item);
  if (ing.note) {
    parts.push(`(${ing.note})`);
  }
  return parts.join(" ");
}

function formatAmount(ing: Ingredient): string | undefined {
  const parts: string[] = [];
  if (ing.measurements[0]?.amount) {
    parts.push(ing.measurements[0].amount);
  }
  if (ing.measurements[0]?.unit) {
    parts.push(ing.measurements[0].unit);
  }
  return parts.length > 0 ? parts.join(" ") : undefined;
}

export default function AddToShoppingListModal(
  props: AddToShoppingListModalProps,
) {
  const { getShoppingListApi } = useAuth();

  const [selectedIndices, setSelectedIndices] = createSignal<Set<number>>(
    new Set(),
  );
  const [adding, setAdding] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [showSuccess, setShowSuccess] = createSignal(false);

  const ingredients = () => props.recipe.ingredients ?? [];

  // Select all ingredients by default when modal opens
  createEffect(() => {
    if (props.isOpen()) {
      const allIndices = new Set(ingredients().map((_, i) => i));
      setSelectedIndices(allIndices);
      setError(null);
      setShowSuccess(false);
    }
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

    setAdding(true);
    setError(null);
    try {
      const items = ingredients()
        .filter((_, i) => selected.has(i))
        .map((ing) => ({
          item: ing.item,
          amount: formatAmount(ing),
          sourceRecipeId: props.recipe.id,
          sourceRecipeTitle: props.recipe.title,
        }));

      await getShoppingListApi().createItems({
        createShoppingListRequest: { items },
      });

      setShowSuccess(true);
      setTimeout(() => {
        props.onClose();
      }, 1500);
    } catch (err) {
      const message = await extractApiError(
        err,
        "Failed to add items to shopping list",
      );
      setError(message);
    } finally {
      setAdding(false);
    }
  };

  const selectedCount = () => selectedIndices().size;

  return (
    <Modal
      isOpen={props.isOpen}
      onClose={props.onClose}
      title="Add to Shopping List"
      actions={
        <Show when={!showSuccess()}>
          <button class="btn" onClick={props.onClose} disabled={adding()}>
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
                  <span class="ingredient-text">{formatIngredient(ing)}</span>
                </label>
              )}
            </For>
          </div>
        </Show>
      </div>
    </Modal>
  );
}
