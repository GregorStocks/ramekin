import { createSignal, For, Show } from "solid-js";
import { useAuth } from "../context/AuthContext";
import Modal from "./Modal";
import { createSubmitFlow } from "../utils/submitFlow";
import {
  formatIngredient,
  formatIngredientAmount,
} from "../utils/ingredientFormatting";
import { logger } from "../utils/logger";
import { syncShoppingList } from "../utils/shoppingListSyncCache";
import type { RecipeResponse, ShoppingListOp } from "ramekin-client";

interface AddToShoppingListModalProps {
  isOpen: () => boolean;
  onClose: () => void;
  recipe: RecipeResponse;
  scale?: () => number;
}

export default function AddToShoppingListModal(
  props: AddToShoppingListModalProps,
) {
  const { getSyncApi, token } = useAuth();

  const [selectedIndices, setSelectedIndices] = createSignal<Set<number>>(
    new Set(),
  );
  // How many items the last submission added, for the confirmation.
  const [addedCount, setAddedCount] = createSignal(0);

  const scale = () => props.scale?.() ?? 1;
  const ingredients = () => props.recipe.ingredients ?? [];

  const flow = createSubmitFlow({
    isOpen: props.isOpen,
    onClose: props.onClose,
    // Select every ingredient on each open.
    onOpen: () => setSelectedIndices(new Set(ingredients().map((_, i) => i))),
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

  const handleAdd = () => {
    const selected = selectedIndices();
    if (selected.size === 0) return;

    return flow.submit("Failed to add items to shopping list", async () => {
      const ops = ingredients()
        .filter((_, i) => selected.has(i))
        .map((ing): ShoppingListOp => ({
          op: "create",
          id: crypto.randomUUID(),
          fields: {
            item: ing.item,
            amount: formatIngredientAmount(ing, scale()),
            sourceRecipeId: props.recipe.id,
            sourceRecipeTitle: props.recipe.title,
          },
        }));

      await syncShoppingList(
        getSyncApi(),
        localStorage,
        token(),
        ops,
        (context, err) => logger.warn("Shopping", `${context}: ${String(err)}`),
      );
      setAddedCount(ops.length);
      return null;
    });
  };

  const selectedCount = () => selectedIndices().size;

  return (
    <Modal
      isOpen={props.isOpen}
      onClose={flow.close}
      title="Add to Shopping List"
      actions={
        <Show when={!flow.confirmed()}>
          <button class="btn" onClick={flow.close} disabled={flow.submitting()}>
            Cancel
          </button>
          <button
            class="btn btn-primary"
            onClick={handleAdd}
            disabled={flow.submitting() || selectedCount() === 0}
          >
            {flow.submitting()
              ? "Adding..."
              : `Add ${selectedCount()} item${selectedCount() !== 1 ? "s" : ""}`}
          </button>
        </Show>
      }
    >
      <div class="add-shopping-modal">
        <Show when={flow.confirmed()}>
          <div class="add-shopping-success">
            Added {addedCount()} item{addedCount() !== 1 ? "s" : ""} to shopping
            list!
          </div>
        </Show>

        <Show when={!flow.confirmed()}>
          <Show when={flow.error()}>
            <div class="error-message" style={{ "margin-bottom": "1rem" }}>
              {flow.error()}
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
