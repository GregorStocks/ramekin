import type { Ingredient } from "ramekin-client";
import { ErrorCode } from "ramekin-client";

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
