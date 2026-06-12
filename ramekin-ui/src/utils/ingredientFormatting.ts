// Web counterpart of ramekin-ios/Ramekin/IngredientFormatting.swift. Keep the
// two in sync (see doc/client-logic-sharing.md) until the shared-test-vector
// harness pins them.
import type { Ingredient, Measurement } from "ramekin-client";

import { scaleAmount } from "./scaleAmount";

export interface FormatIngredientOptions {
  scale?: number;
  includeAlternatives?: boolean;
  includeNote?: boolean;
}

/**
 * The pieces of a formatted ingredient line, kept separate so the recipe view
 * can style each one. `formatIngredient` joins them in display order:
 * "<amount> <unit> (<alternatives>) <item> (<note>)".
 */
export interface IngredientParts {
  amount: string | null;
  unit: string | null;
  /** Alternative measurements joined with ", " — no surrounding parens. */
  alternatives: string | null;
  item: string;
  /** Trimmed note — no surrounding parens. */
  note: string | null;
}

function trimmedValue(value: string | null | undefined): string | null {
  const trimmed = value?.trim();
  return trimmed ? trimmed : null;
}

function formatMeasurement(
  measurement: Measurement,
  scale: number,
): string | null {
  const amount = trimmedValue(scaleAmount(measurement.amount, scale));
  const unit = trimmedValue(measurement.unit);
  const values = [amount, unit].filter((v): v is string => v !== null);
  return values.length > 0 ? values.join(" ") : null;
}

export function formatIngredientParts(
  ingredient: Ingredient,
  options: FormatIngredientOptions = {},
): IngredientParts {
  const scale = options.scale ?? 1;
  const primary = ingredient.measurements[0];

  let alternatives: string | null = null;
  if (options.includeAlternatives) {
    const formatted = ingredient.measurements
      .slice(1)
      .map((m) => formatMeasurement(m, scale))
      .filter((v): v is string => v !== null);
    alternatives = formatted.length > 0 ? formatted.join(", ") : null;
  }

  return {
    amount: primary ? trimmedValue(scaleAmount(primary.amount, scale)) : null,
    unit: primary ? trimmedValue(primary.unit) : null,
    alternatives,
    item: ingredient.item,
    note: options.includeNote ? trimmedValue(ingredient.note) : null,
  };
}

/** Mirrors `Ingredient.formatted(scale:includeAlternatives:includeNote:)`. */
export function formatIngredient(
  ingredient: Ingredient,
  options: FormatIngredientOptions = {},
): string {
  const parts = formatIngredientParts(ingredient, options);

  const out: string[] = [];
  const primary = [parts.amount, parts.unit].filter(Boolean).join(" ");
  if (primary) {
    out.push(primary);
  }
  if (parts.alternatives) {
    out.push(`(${parts.alternatives})`);
  }
  out.push(parts.item);
  if (parts.note) {
    out.push(`(${parts.note})`);
  }
  return out.join(" ");
}

/**
 * Primary measurement only ("<amount> <unit>"), for the shopping-list item
 * payload. Mirrors `AddToShoppingListSheetSupport.formattedAmount`.
 */
export function formatIngredientAmount(
  ingredient: Ingredient,
  scale: number,
): string | undefined {
  const primary = ingredient.measurements[0];
  if (!primary) {
    return undefined;
  }
  return formatMeasurement(primary, scale) ?? undefined;
}
