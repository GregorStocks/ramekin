import { For, Show } from "solid-js";
import DiffViewer from "./DiffViewer";
import type { NormalizedRecipe, RecipeContent } from "ramekin-client";
import { formatIngredients, formatTags } from "../utils/formatRecipeForDiff";

interface RecipeContentDiffProps {
  oldContent: RecipeContent;
  newContent: RecipeContent;
  /** Shown when nothing differs. */
  emptyMessage: string;
}

const FIELDS: { label: string; text: (content: RecipeContent) => string }[] = [
  { label: "Title", text: (c) => c.title },
  { label: "Description", text: (c) => c.description ?? "" },
  { label: "Ingredients", text: (c) => formatIngredients(c.ingredients) },
  { label: "Instructions", text: (c) => c.instructions },
  { label: "Tags", text: (c) => formatTags(c.tags) },
  { label: "Notes", text: (c) => c.notes ?? "" },
  { label: "Prep Time", text: (c) => c.prepTime ?? "" },
  { label: "Cook Time", text: (c) => c.cookTime ?? "" },
  { label: "Total Time", text: (c) => c.totalTime ?? "" },
  { label: "Servings", text: (c) => c.servings ?? "" },
  { label: "Difficulty", text: (c) => c.difficulty ?? "" },
  { label: "Nutritional Info", text: (c) => c.nutritionalInfo ?? "" },
  { label: "Source", text: (c) => c.sourceName ?? "" },
  { label: "Source URL", text: (c) => c.sourceUrl ?? "" },
];

/** The fields that differ between two recipe contents, in reading order. */
export function recipeContentChanges(
  oldContent: RecipeContent,
  newContent: RecipeContent,
) {
  return FIELDS.map(({ label, text }) => ({
    label,
    oldText: text(oldContent),
    newText: text(newContent),
  })).filter((change) => change.oldText !== change.newText);
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

/** Field-by-field diff of two recipe contents, showing only what changed. */
export default function RecipeContentDiff(props: RecipeContentDiffProps) {
  const changes = () =>
    recipeContentChanges(props.oldContent, props.newContent);

  return (
    <div class="recipe-content-diff">
      <For each={changes()}>
        {(change) => (
          <DiffViewer
            label={change.label}
            oldText={change.oldText}
            newText={change.newText}
          />
        )}
      </For>
      <Show when={changes().length === 0}>
        <p class="no-changes">{props.emptyMessage}</p>
      </Show>
    </div>
  );
}
