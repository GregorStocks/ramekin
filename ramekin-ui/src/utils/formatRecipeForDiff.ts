// Mirrors ramekin-ios/Ramekin/RecipeVersionSupport.swift formatIngredients/
// formatTags. Keep the two in sync (see doc/client-logic-sharing.md) until
// the shared-test-vector harness pins them.
import type { Ingredient } from "ramekin-client";

import { formatIngredient } from "./ingredientFormatting";

export function formatIngredients(ingredients: Ingredient[]): string {
  const lines: string[] = [];
  let currentSection: string | null = null;

  for (const ingredient of ingredients) {
    const section = ingredient.section ?? null;
    if (section !== currentSection) {
      currentSection = section;
      if (currentSection) {
        lines.push(`[${currentSection}]`);
      }
    }

    lines.push(
      formatIngredient(ingredient, {
        includeAlternatives: true,
        includeNote: true,
      }),
    );
  }

  return lines.join("\n");
}

export function formatTags(tags: string[] | undefined | null): string {
  return tags?.join(", ") || "";
}
