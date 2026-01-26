import type { SetStoreFunction } from "solid-js/store";
import type { Ingredient } from "ramekin-client";

export function addIngredient(
  ingredients: Ingredient[],
  setIngredients: SetStoreFunction<Ingredient[]>,
) {
  setIngredients(ingredients.length, { item: "", amount: "", unit: "" });
}

export function removeIngredient(
  index: number,
  setIngredients: SetStoreFunction<Ingredient[]>,
) {
  setIngredients((ings) => ings.filter((_, i) => i !== index));
}

export function updateIngredient(
  index: number,
  field: keyof Ingredient,
  value: string,
  setIngredients: SetStoreFunction<Ingredient[]>,
) {
  setIngredients(index, field, value);
}

/**
 * Extract error message from API response errors.
 * Handles both direct Response objects and objects with a response property
 * (like the generated client's ResponseError).
 */
export async function extractApiError(
  err: unknown,
  fallbackMessage: string,
): Promise<string> {
  const response =
    err instanceof Response
      ? err
      : err &&
          typeof err === "object" &&
          "response" in err &&
          err.response instanceof Response
        ? err.response
        : null;

  if (response) {
    try {
      const body = await response.json();
      return body.error || fallbackMessage;
    } catch {
      return `${fallbackMessage} (${response.status})`;
    }
  }

  return fallbackMessage;
}
