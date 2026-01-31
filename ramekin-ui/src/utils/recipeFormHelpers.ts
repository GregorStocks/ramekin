import type { SetStoreFunction } from "solid-js/store";
import type { Ingredient } from "ramekin-client";

export function addIngredient(
  ingredients: Ingredient[],
  setIngredients: SetStoreFunction<Ingredient[]>,
) {
  setIngredients(ingredients.length, { item: "", measurements: [{}] });
}

export function removeIngredient(
  index: number,
  setIngredients: SetStoreFunction<Ingredient[]>,
) {
  setIngredients((ings) => ings.filter((_, i) => i !== index));
}

export function updateIngredientItem(
  index: number,
  value: string,
  setIngredients: SetStoreFunction<Ingredient[]>,
) {
  setIngredients(index, "item", value);
}

export function updateIngredientNote(
  index: number,
  value: string | undefined,
  setIngredients: SetStoreFunction<Ingredient[]>,
) {
  setIngredients(index, "note", value || undefined);
}

export function updateIngredientAmount(
  index: number,
  value: string,
  setIngredients: SetStoreFunction<Ingredient[]>,
) {
  setIngredients(index, "measurements", 0, "amount", value || undefined);
}

export function updateIngredientUnit(
  index: number,
  value: string,
  setIngredients: SetStoreFunction<Ingredient[]>,
) {
  setIngredients(index, "measurements", 0, "unit", value || undefined);
}

/**
 * Get the primary measurement's amount from an ingredient.
 */
export function getAmount(ing: Ingredient): string {
  return ing.measurements[0]?.amount || "";
}

/**
 * Get the primary measurement's unit from an ingredient.
 */
export function getUnit(ing: Ingredient): string {
  return ing.measurements[0]?.unit || "";
}

/**
 * Add an alternative measurement to an ingredient.
 */
export function addAlternativeMeasurement(
  ingredientIndex: number,
  setIngredients: SetStoreFunction<Ingredient[]>,
) {
  setIngredients(ingredientIndex, "measurements", (measurements) => [
    ...measurements,
    {},
  ]);
}

/**
 * Remove a measurement from an ingredient (must have at least one measurement).
 */
export function removeMeasurement(
  ingredientIndex: number,
  measurementIndex: number,
  setIngredients: SetStoreFunction<Ingredient[]>,
) {
  setIngredients(ingredientIndex, "measurements", (measurements) =>
    measurements.filter((_, i) => i !== measurementIndex),
  );
}

/**
 * Update a specific measurement's amount.
 */
export function updateMeasurementAmount(
  ingredientIndex: number,
  measurementIndex: number,
  value: string,
  setIngredients: SetStoreFunction<Ingredient[]>,
) {
  setIngredients(
    ingredientIndex,
    "measurements",
    measurementIndex,
    "amount",
    value || undefined,
  );
}

/**
 * Update a specific measurement's unit.
 */
export function updateMeasurementUnit(
  ingredientIndex: number,
  measurementIndex: number,
  value: string,
  setIngredients: SetStoreFunction<Ingredient[]>,
) {
  setIngredients(
    ingredientIndex,
    "measurements",
    measurementIndex,
    "unit",
    value || undefined,
  );
}

/**
 * Get a specific measurement's amount.
 */
export function getMeasurementAmount(
  ing: Ingredient,
  measurementIndex: number,
): string {
  return ing.measurements[measurementIndex]?.amount || "";
}

/**
 * Get a specific measurement's unit.
 */
export function getMeasurementUnit(
  ing: Ingredient,
  measurementIndex: number,
): string {
  return ing.measurements[measurementIndex]?.unit || "";
}

/**
 * Update an ingredient's section.
 */
export function updateIngredientSection(
  index: number,
  value: string | undefined,
  setIngredients: SetStoreFunction<Ingredient[]>,
) {
  setIngredients(index, "section", value || undefined);
}

/**
 * Add a new ingredient with a section (for starting a new section).
 */
export function addIngredientWithSection(
  ingredients: Ingredient[],
  setIngredients: SetStoreFunction<Ingredient[]>,
  section: string,
) {
  setIngredients(ingredients.length, {
    item: "",
    measurements: [{}],
    section,
  });
}

/**
 * Move an ingredient from one position to another, updating sections as needed.
 */
export function moveIngredient(
  fromIndex: number,
  toIndex: number,
  newSection: string | undefined,
  ingredients: Ingredient[],
  setIngredients: SetStoreFunction<Ingredient[]>,
) {
  const updated = [...ingredients];
  const [moved] = updated.splice(fromIndex, 1);
  moved.section = newSection;
  updated.splice(toIndex > fromIndex ? toIndex - 1 : toIndex, 0, moved);
  setIngredients(updated);
}

/** Group ingredients by contiguous sections (preserving order). */
export function groupIngredientsBySection(ingredients: Ingredient[]): Array<{
  section: string | null;
  ingredients: Ingredient[];
  startIndex: number;
}> {
  const groups: Array<{
    section: string | null;
    ingredients: Ingredient[];
    startIndex: number;
  }> = [];
  let currentIndex = 0;

  for (const ing of ingredients) {
    const section = ing.section ?? null;
    const lastGroup = groups[groups.length - 1];

    if (lastGroup && lastGroup.section === section) {
      lastGroup.ingredients.push(ing);
    } else {
      groups.push({ section, ingredients: [ing], startIndex: currentIndex });
    }
    currentIndex++;
  }

  return groups;
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
