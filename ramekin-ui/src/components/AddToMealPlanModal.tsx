import { createSignal, For, Show, type Accessor } from "solid-js";
import type { MealType } from "ramekin-client";
import { useAuth } from "../context/AuthContext";
import {
  MEAL_TYPES,
  MEAL_TYPE_LABELS,
  formatDateLocal,
  parseLocalDate,
  toApiDate,
} from "../utils/mealPlanHelpers";
import {
  createMealPlanOp,
  mealPlanOpError,
  writeMealPlan,
} from "../utils/mealPlanWrites";
import { createSubmitFlow } from "../utils/submitFlow";
import Modal from "./Modal";

interface AddToMealPlanModalProps {
  isOpen: Accessor<boolean>;
  onClose: () => void;
  recipeId: string;
}

export default function AddToMealPlanModal(props: AddToMealPlanModalProps) {
  const { getSyncApi, token } = useAuth();
  const [mealPlanDate, setMealPlanDate] = createSignal(
    formatDateLocal(new Date()),
  );
  const [mealPlanMealType, setMealPlanMealType] =
    createSignal<MealType>("dinner");
  const flow = createSubmitFlow({
    isOpen: props.isOpen,
    onClose: props.onClose,
    onOpen: () => {
      setMealPlanDate(formatDateLocal(new Date()));
      setMealPlanMealType("dinner");
    },
  });

  const handleAddToMealPlan = () =>
    flow.submit("Failed to add to meal plan", async () => {
      const mealType = mealPlanMealType();
      const result = await writeMealPlan(
        getSyncApi(),
        token(),
        createMealPlanOp({
          recipeId: props.recipeId,
          mealDate: toApiDate(parseLocalDate(mealPlanDate())),
          mealType,
        }),
      );
      return mealPlanOpError(result, mealType);
    });

  return (
    <Modal
      isOpen={props.isOpen}
      onClose={flow.close}
      title="Add to Meal Plan"
      actions={
        <>
          <button
            type="button"
            class="btn"
            onClick={flow.close}
            disabled={flow.submitting()}
          >
            Cancel
          </button>
          <button
            type="button"
            class="btn btn-primary"
            onClick={handleAddToMealPlan}
            disabled={flow.submitting() || flow.confirmed()}
          >
            {flow.submitting() ? "Adding..." : "Add"}
          </button>
        </>
      }
    >
      <Show when={flow.confirmed()}>
        <div class="meal-plan-success">Added to meal plan!</div>
      </Show>
      <Show when={!flow.confirmed()}>
        <div class="meal-plan-form">
          <div class="form-group">
            <label for="meal-plan-date">Date</label>
            <input
              type="date"
              id="meal-plan-date"
              value={mealPlanDate()}
              onInput={(e) => setMealPlanDate(e.currentTarget.value)}
            />
          </div>
          <div class="form-group">
            <label>Meal</label>
            <div class="meal-type-buttons">
              <For each={MEAL_TYPES}>
                {(type) => (
                  <button
                    type="button"
                    class={`meal-type-button ${mealPlanMealType() === type ? "selected" : ""}`}
                    onClick={() => setMealPlanMealType(type)}
                  >
                    {MEAL_TYPE_LABELS[type]}
                  </button>
                )}
              </For>
            </div>
          </div>
          <Show when={flow.error()}>
            <p class="error">{flow.error()}</p>
          </Show>
        </div>
      </Show>
    </Modal>
  );
}
