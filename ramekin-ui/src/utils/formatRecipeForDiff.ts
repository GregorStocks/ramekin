import type { Ingredient } from "ramekin-client";

export function formatIngredients(ingredients: Ingredient[]): string {
  return ingredients
    .map((ing) => {
      const primary = ing.measurements[0];
      const altMeasurements =
        ing.measurements.length > 1
          ? `(${ing.measurements
              .slice(1)
              .map((m) => [m.amount, m.unit].filter(Boolean).join(" "))
              .join(", ")})`
          : "";
      return [
        primary?.amount,
        primary?.unit,
        altMeasurements,
        ing.item,
        ing.note ? `(${ing.note})` : "",
      ]
        .filter(Boolean)
        .join(" ");
    })
    .join("\n");
}

export function formatTags(tags: string[] | undefined | null): string {
  return tags?.join(", ") || "";
}
