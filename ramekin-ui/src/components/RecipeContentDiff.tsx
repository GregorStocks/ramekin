import { For, Show } from "solid-js";
import DiffViewer from "./DiffViewer";
import type { RecipeContent } from "ramekin-client";
import { recipeContentChanges } from "../utils/recipeContentChanges";

interface RecipeContentDiffProps {
  oldContent: RecipeContent;
  newContent: RecipeContent;
  /** Shown when nothing differs. */
  emptyMessage: string;
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
