// Mirrors ramekin-ios/Ramekin/RecipeContentDiffView.swift
// (RecipeContentFieldDiff); pinned by
// shared-test-vectors/normalize-selection.json (proposal_changes).
import type {
  Ingredient,
  NormalizedRecipe,
  RecipeContent,
} from "ramekin-client";
import { formatIngredients, formatTags } from "./formatRecipeForDiff";

export interface RecipeContentChange {
  label: string;
  oldText: string;
  newText: string;
}

/**
 * Ingredients as their parsed parts, one per line, so a re-parse that reads
 * the same ("1 cup water" as the item vs. amount 1, unit cup, item water)
 * still shows as a change.
 */
export function formatIngredientStructure(ingredients: Ingredient[]): string {
  return ingredients
    .map((ingredient) => {
      const section = ingredient.section ? `[${ingredient.section}] ` : "";
      const measurements = ingredient.measurements
        .map((m) => `${m.amount ?? ""} ${m.unit ?? ""}`.trim())
        .join(" / ");
      const note = ingredient.note ? ` | ${ingredient.note}` : "";
      return `${section}${ingredient.item} | ${measurements}${note}`;
    })
    .join("\n");
}

const FIELDS: { label: string; text: (content: RecipeContent) => string }[] = [
  { label: "Title", text: (c) => c.title },
  { label: "Description", text: (c) => c.description ?? "" },
  { label: "Ingredients", text: (c) => formatIngredients(c.ingredients) },
  {
    label: "Ingredients (parsed)",
    text: (c) => formatIngredientStructure(c.ingredients),
  },
  { label: "Instructions", text: (c) => c.instructions },
  { label: "Tags", text: (c) => formatTags(c.tags) },
  { label: "Notes", text: (c) => c.notes ?? "" },
  { label: "Prep Time", text: (c) => c.prepTime ?? "" },
  { label: "Cook Time", text: (c) => c.cookTime ?? "" },
  { label: "Total Time", text: (c) => c.totalTime ?? "" },
  { label: "Servings", text: (c) => c.servings ?? "" },
  { label: "Difficulty", text: (c) => c.difficulty ?? "" },
  { label: "Rating", text: (c) => (c.rating == null ? "" : String(c.rating)) },
  { label: "Nutritional Info", text: (c) => c.nutritionalInfo ?? "" },
  { label: "Source", text: (c) => c.sourceName ?? "" },
  { label: "Source URL", text: (c) => c.sourceUrl ?? "" },
];

/**
 * The fields that differ between two recipe contents, in reading order.
 * "Ingredients (parsed)" appears only when the ingredients read the same but
 * were parsed differently.
 */
export function recipeContentChanges(
  oldContent: RecipeContent,
  newContent: RecipeContent,
): RecipeContentChange[] {
  const changes = FIELDS.map(({ label, text }) => ({
    label,
    oldText: text(oldContent),
    newText: text(newContent),
  })).filter((change) => change.oldText !== change.newText);
  return changes.some((change) => change.label === "Ingredients")
    ? changes.filter((change) => change.label !== "Ingredients (parsed)")
    : changes;
}

/** Whether accepting `proposed` over `base` would change anything. */
export function normalizedRecipeChanged(
  base: NormalizedRecipe,
  proposed: NormalizedRecipe,
): boolean {
  return (
    recipeContentChanges(base.content, proposed.content).length > 0 ||
    base.photoIds.join(",") !== proposed.photoIds.join(",")
  );
}
