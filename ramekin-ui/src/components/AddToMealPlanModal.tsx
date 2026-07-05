import {
  createEffect,
  createSignal,
  For,
  onCleanup,
  Show,
  type Accessor,
} from "solid-js";
import { ErrorCode, type MealType } from "ramekin-client";
import { useAuth } from "../context/AuthContext";
import {
  MEAL_TYPES,
  MEAL_TYPE_LABELS,
  formatDateLocal,
  parseLocalDate,
  toApiDate,
} from "../utils/mealPlanHelpers";
import { parseApiError } from "../utils/recipeFormHelpers";
import Modal from "./Modal";

interface AddToMealPlanModalProps {
  isOpen: Accessor<boolean>;
  onClose: () => void;
  recipeId: string;
}

export default function AddToMealPlanModal(props: AddToMealPlanModalProps) {
  const { getMealPlansApi } = useAuth();
  const [mealPlanDate, setMealPlanDate] = createSignal(
    formatDateLocal(new Date()),
  );
  const [mealPlanMealType, setMealPlanMealType] =
    createSignal<MealType>("dinner");
  const [addingToMealPlan, setAddingToMealPlan] = createSignal(false);
  const [mealPlanError, setMealPlanError] = createSignal<string | null>(null);
  const [mealPlanSuccess, setMealPlanSuccess] = createSignal(false);
  const [wasOpen, setWasOpen] = createSignal(false);
  let closeTimer: ReturnType<typeof setTimeout> | null = null;

  const clearCloseTimer = () => {
    if (closeTimer) {
      clearTimeout(closeTimer);
      closeTimer = null;
    }
  };

  const reset = () => {
    clearCloseTimer();
    setMealPlanDate(formatDateLocal(new Date()));
    setMealPlanMealType("dinner");
    setMealPlanError(null);
    setMealPlanSuccess(false);
  };

  const close = () => {
    clearCloseTimer();
    props.onClose();
    setMealPlanError(null);
    setMealPlanSuccess(false);
  };

  createEffect(() => {
    const open = props.isOpen();
    if (open && !wasOpen()) reset();
    setWasOpen(open);
  });

  onCleanup(() => {
    clearCloseTimer();
  });

  const handleAddToMealPlan = async () => {
    setAddingToMealPlan(true);
    setMealPlanError(null);
    try {
      await getMealPlansApi().createMealPlan({
        createMealPlanRequest: {
          recipeId: props.recipeId,
          mealDate: toApiDate(parseLocalDate(mealPlanDate())),
          mealType: mealPlanMealType(),
        },
      });
      setMealPlanSuccess(true);
      clearCloseTimer();
      closeTimer = setTimeout(close, 1500);
    } catch (err) {
      const parsed = await parseApiError(err, "Failed to add to meal plan");
      if (parsed.code === ErrorCode.Conflict) {
        setMealPlanError(
          `This recipe is already scheduled for ${MEAL_TYPE_LABELS[mealPlanMealType()].toLowerCase()} on this date`,
        );
      } else {
        setMealPlanError(parsed.message);
      }
    } finally {
      setAddingToMealPlan(false);
    }
  };

  return (
    <Modal
      isOpen={props.isOpen}
      onClose={close}
      title="Add to Meal Plan"
      actions={
        <>
          <button
            type="button"
            class="btn"
            onClick={close}
            disabled={addingToMealPlan()}
          >
            Cancel
          </button>
          <button
            type="button"
            class="btn btn-primary"
            onClick={handleAddToMealPlan}
            disabled={addingToMealPlan() || mealPlanSuccess()}
          >
            {addingToMealPlan() ? "Adding..." : "Add"}
          </button>
        </>
      }
    >
      <Show when={mealPlanSuccess()}>
        <div class="meal-plan-success">Added to meal plan!</div>
      </Show>
      <Show when={!mealPlanSuccess()}>
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
          <Show when={mealPlanError()}>
            <p class="error">{mealPlanError()}</p>
          </Show>
        </div>
      </Show>
    </Modal>
  );
}
