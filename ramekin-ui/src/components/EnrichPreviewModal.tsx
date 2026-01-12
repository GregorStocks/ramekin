import { Show } from "solid-js";
import Modal from "./Modal";
import DiffViewer from "./DiffViewer";
import type { RecipeResponse, RecipeContent } from "ramekin-client";

interface EnrichPreviewModalProps {
  isOpen: () => boolean;
  onClose: () => void;
  currentRecipe: RecipeResponse;
  enrichedContent: RecipeContent;
  onApply: () => void;
  applying: boolean;
}

export default function EnrichPreviewModal(props: EnrichPreviewModalProps) {
  const hasChanges = (field: keyof RecipeContent) => {
    const current = props.currentRecipe[field as keyof RecipeResponse];
    const enriched = props.enrichedContent[field];
    return current !== enriched;
  };

  const formatIngredients = (
    ingredients: Array<{
      amount?: string | null;
      unit?: string | null;
      item: string;
      note?: string | null;
    }>,
  ) => {
    return ingredients
      .map((ing) =>
        [ing.amount, ing.unit, ing.item, ing.note ? `(${ing.note})` : ""]
          .filter(Boolean)
          .join(" "),
      )
      .join("\n");
  };

  const ingredientsChanged = () => {
    const currentStr = formatIngredients(props.currentRecipe.ingredients);
    const enrichedStr = formatIngredients(props.enrichedContent.ingredients);
    return currentStr !== enrichedStr;
  };

  const tagsChanged = () => {
    const currentTags = props.currentRecipe.tags?.join(", ") || "";
    const enrichedTags = props.enrichedContent.tags?.join(", ") || "";
    return currentTags !== enrichedTags;
  };

  return (
    <Modal
      isOpen={props.isOpen}
      onClose={props.onClose}
      title="AI Enrichment Suggestions"
      actions={
        <>
          <button type="button" class="btn" onClick={props.onClose}>
            Cancel
          </button>
          <button
            type="button"
            class="btn btn-primary"
            onClick={props.onApply}
            disabled={props.applying}
          >
            {props.applying ? "Applying..." : "Apply Changes"}
          </button>
        </>
      }
    >
      <div class="enrich-preview">
        <Show when={hasChanges("title")}>
          <DiffViewer
            label="Title"
            oldText={props.currentRecipe.title}
            newText={props.enrichedContent.title}
          />
        </Show>

        <Show when={hasChanges("description")}>
          <DiffViewer
            label="Description"
            oldText={props.currentRecipe.description || ""}
            newText={props.enrichedContent.description || ""}
          />
        </Show>

        <Show when={ingredientsChanged()}>
          <DiffViewer
            label="Ingredients"
            oldText={formatIngredients(props.currentRecipe.ingredients)}
            newText={formatIngredients(props.enrichedContent.ingredients)}
          />
        </Show>

        <Show when={hasChanges("instructions")}>
          <DiffViewer
            label="Instructions"
            oldText={props.currentRecipe.instructions}
            newText={props.enrichedContent.instructions}
          />
        </Show>

        <Show when={tagsChanged()}>
          <DiffViewer
            label="Tags"
            oldText={props.currentRecipe.tags?.join(", ") || ""}
            newText={props.enrichedContent.tags?.join(", ") || ""}
          />
        </Show>

        <Show when={hasChanges("notes")}>
          <DiffViewer
            label="Notes"
            oldText={props.currentRecipe.notes || ""}
            newText={props.enrichedContent.notes || ""}
          />
        </Show>

        <Show when={hasChanges("prepTime")}>
          <DiffViewer
            label="Prep Time"
            oldText={props.currentRecipe.prepTime || ""}
            newText={props.enrichedContent.prepTime || ""}
          />
        </Show>

        <Show when={hasChanges("cookTime")}>
          <DiffViewer
            label="Cook Time"
            oldText={props.currentRecipe.cookTime || ""}
            newText={props.enrichedContent.cookTime || ""}
          />
        </Show>

        <Show when={hasChanges("totalTime")}>
          <DiffViewer
            label="Total Time"
            oldText={props.currentRecipe.totalTime || ""}
            newText={props.enrichedContent.totalTime || ""}
          />
        </Show>

        <Show when={hasChanges("servings")}>
          <DiffViewer
            label="Servings"
            oldText={props.currentRecipe.servings || ""}
            newText={props.enrichedContent.servings || ""}
          />
        </Show>

        <Show when={hasChanges("difficulty")}>
          <DiffViewer
            label="Difficulty"
            oldText={props.currentRecipe.difficulty || ""}
            newText={props.enrichedContent.difficulty || ""}
          />
        </Show>

        <Show when={hasChanges("nutritionalInfo")}>
          <DiffViewer
            label="Nutritional Info"
            oldText={props.currentRecipe.nutritionalInfo || ""}
            newText={props.enrichedContent.nutritionalInfo || ""}
          />
        </Show>

        <Show
          when={
            !hasChanges("title") &&
            !hasChanges("description") &&
            !ingredientsChanged() &&
            !hasChanges("instructions") &&
            !tagsChanged() &&
            !hasChanges("notes") &&
            !hasChanges("prepTime") &&
            !hasChanges("cookTime") &&
            !hasChanges("totalTime") &&
            !hasChanges("servings") &&
            !hasChanges("difficulty") &&
            !hasChanges("nutritionalInfo")
          }
        >
          <p class="no-changes">No changes suggested by AI enrichment.</p>
        </Show>
      </div>
    </Modal>
  );
}
