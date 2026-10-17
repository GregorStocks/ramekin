import type {
  MealPlanFields,
  MealPlanOp,
  MealType,
  OpResult,
  SyncApi,
} from "ramekin-client";
import { MEAL_TYPE_LABELS } from "./mealPlanHelpers";

// Web reads meal plans a week at a time and doesn't cache them, so writes
// ignore the feed in the response. Holding its cursor just keeps each
// response to the rows that changed rather than every plan.
let cursor: number | undefined;

/** Send one meal-plan op through the sync endpoint. */
export async function writeMealPlan(
  api: Pick<SyncApi, "syncMealPlans">,
  op: MealPlanOp,
): Promise<OpResult> {
  const response = await api.syncMealPlans({
    syncMealPlansRequest: { cursor, ops: [op] },
  });
  cursor = response.cursor;
  return response.results[0];
}

export function createMealPlanOp(fields: MealPlanFields): MealPlanOp {
  return { op: "create", id: crypto.randomUUID(), fields };
}

/** What to tell the user when an op didn't apply, or null when it did. */
export function mealPlanOpError(
  result: OpResult,
  mealType: MealType,
): string | null {
  switch (result) {
    case "applied":
      return null;
    case "duplicate":
      return `This recipe is already scheduled for ${MEAL_TYPE_LABELS[mealType].toLowerCase()} on this date`;
    case "recipe_missing":
      return "This recipe no longer exists";
    case "gone":
      return "This meal was already removed";
  }
}
