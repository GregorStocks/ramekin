import type { SetStoreFunction } from "solid-js/store";
import type { Ingredient } from "ramekin-client";
import { ErrorCode } from "ramekin-client";

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

/** A parsed API error: machine-readable code, human message, and HTTP status. */
export interface ParsedApiError {
  /** Machine-readable error code, or null if the body wasn't a structured error. */
  code: ErrorCode | null;
  /** Human-readable message for display. Never branch on this. */
  message: string;
  /** HTTP status, or null if the error wasn't an HTTP response. */
  status: number | null;
}

export function recipeUpdateErrorMessage(error: ParsedApiError): string {
  return error.code === ErrorCode.Conflict
    ? "This recipe changed since you opened it. Your edits are still here; reload before saving again."
    : error.message;
}

/**
 * Parse an API error into its structured `code`, human-readable `message`, and
 * HTTP `status`. Branch on `code` (against the {@link ErrorCode} enum), never on
 * the message text. Handles both direct `Response` objects and the generated
 * client's `ResponseError` (which wraps the response).
 *
 * The response body is consumed here, so call this at most once per caught error.
 */
export async function parseApiError(
  err: unknown,
  fallbackMessage: string,
): Promise<ParsedApiError> {
  const response =
    err instanceof Response
      ? err
      : err &&
          typeof err === "object" &&
          "response" in err &&
          err.response instanceof Response
        ? err.response
        : null;

  if (!response) {
    return { code: null, message: fallbackMessage, status: null };
  }

  try {
    const body = await response.json();
    return {
      code: typeof body.code === "string" ? (body.code as ErrorCode) : null,
      message: body.error || fallbackMessage,
      status: response.status,
    };
  } catch {
    return {
      code: null,
      message: `${fallbackMessage} (${response.status})`,
      status: response.status,
    };
  }
}

/**
 * Extract a human-readable error message from an API error. To branch on the
 * kind of error, use {@link parseApiError} and inspect `.code` instead.
 */
export async function extractApiError(
  err: unknown,
  fallbackMessage: string,
): Promise<string> {
  return (await parseApiError(err, fallbackMessage)).message;
}

/**
 * Pull the first image file out of a ClipboardEvent's data, if any.
 */
export function extractImageFile(data: DataTransfer | null): File | null {
  if (!data) return null;
  for (const item of data.items) {
    if (item.kind === "file" && item.type.startsWith("image/")) {
      const file = item.getAsFile();
      if (file) return file;
    }
  }
  return null;
}
