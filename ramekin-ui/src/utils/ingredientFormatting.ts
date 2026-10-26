// Web counterpart of ramekin-ios/Ramekin/IngredientFormatting.swift. Shared
// vectors pin the two implementations; see doc/client-logic-sharing.md.
import type {
  DerivedMeasurement,
  Ingredient,
  Measurement,
} from "ramekin-client";

import { scaleAmount } from "./scaleAmount";

export interface FormatIngredientOptions {
  scale?: number;
  includeAlternatives?: boolean;
  includeNote?: boolean;
  /**
   * Approximate grams the server computed for this line. When alternatives
   * are included they lead the line as "~120 g", and the stored measurements
   * all move into the parens.
   */
  derived?: Measurement | null;
}

/**
 * The pieces of a formatted ingredient line, kept separate so the recipe view
 * can style each one. `formatIngredient` joins them in display order:
 * "<amount> <unit> (<alternatives>) <item> (<note>)". With derived grams the
 * amount and unit are the grams ("~120" "g") and the alternatives start with
 * the stored primary measurement.
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

/**
 * A per-item size ("10 oz each" in "2 packages (10 oz each)") stays the same
 * however many items the recipe calls for.
 */
function isPerItem(measurement: Measurement): boolean {
  return (measurement.unit?.trim().toLowerCase() ?? "").endsWith(" each");
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
  const [primary, ...rest] = ingredient.measurements;
  // Derived grams lead the line, marked approximate, and push the stored
  // primary into the alternatives. Grams without an amount say nothing.
  const derived = options.includeAlternatives ? options.derived : null;
  const derivedAmount = derived
    ? trimmedValue(scaleAmount(derived.amount, scale))
    : null;
  const lead = derivedAmount ? derived : primary;
  const amount = derivedAmount
    ? `~${derivedAmount}`
    : primary
      ? trimmedValue(scaleAmount(primary.amount, scale))
      : null;

  let alternatives: string | null = null;
  if (options.includeAlternatives) {
    const formatted = [
      derivedAmount && primary ? formatMeasurement(primary, scale) : null,
      ...rest.map((m) => formatMeasurement(m, isPerItem(m) ? 1 : scale)),
    ].filter((v): v is string => !!v);
    alternatives = formatted.length > 0 ? formatted.join(", ") : null;
  }

  return {
    amount,
    unit: lead ? trimmedValue(lead.unit) : null,
    alternatives,
    item: ingredient.item,
    note: options.includeNote ? trimmedValue(ingredient.note) : null,
  };
}

/**
 * Mirrors `Ingredient.formatted(scale:includeAlternatives:includeNote:derived:)`.
 */
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

/** The derived grams the server computed for the ingredient at `index`. */
export function derivedMeasurementAt(
  derived: DerivedMeasurement[],
  index: number,
): Measurement | null {
  return derived.find((d) => d.ingredientIndex === index) ?? null;
}
