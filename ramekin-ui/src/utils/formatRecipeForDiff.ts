import type { Ingredient } from "ramekin-client";

export function formatIngredients(ingredients: Ingredient[]): string {
  return ingredients
    .map((ing) =>
      [ing.amount, ing.unit, ing.item, ing.note ? `(${ing.note})` : ""]
        .filter(Boolean)
        .join(" "),
    )
    .join("\n");
}

export function formatTags(tags: string[] | undefined | null): string {
  return tags?.join(", ") || "";
}
