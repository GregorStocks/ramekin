import { Show } from "solid-js";
import Modal from "./Modal";
import DiffViewer from "./DiffViewer";
import VersionSourceBadge from "./VersionSourceBadge";
import { formatIngredients, formatTags } from "../utils/formatRecipeForDiff";
import type { RecipeResponse } from "ramekin-client";

interface VersionCompareModalProps {
  isOpen: () => boolean;
  onClose: () => void;
  loading: boolean;
  versionA: RecipeResponse | null;
  versionB: RecipeResponse | null;
}

export default function VersionCompareModal(props: VersionCompareModalProps) {
  const formatDate = (date: Date) => {
    return new Intl.DateTimeFormat("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
      hour: "numeric",
      minute: "2-digit",
    }).format(date);
  };

  const hasChanges = (field: keyof RecipeResponse) => {
    if (!props.versionA || !props.versionB) return false;
    return props.versionA[field] !== props.versionB[field];
  };

  const ingredientsChanged = () => {
    if (!props.versionA || !props.versionB) return false;
    const aStr = formatIngredients(props.versionA.ingredients);
    const bStr = formatIngredients(props.versionB.ingredients);
    return aStr !== bStr;
  };

  const tagsChanged = () => {
    if (!props.versionA || !props.versionB) return false;
    const aTags = formatTags(props.versionA.tags);
    const bTags = formatTags(props.versionB.tags);
    return aTags !== bTags;
  };

  const hasAnyChanges = () => {
    return (
      hasChanges("title") ||
      hasChanges("description") ||
      ingredientsChanged() ||
      hasChanges("instructions") ||
      tagsChanged() ||
      hasChanges("notes") ||
      hasChanges("prepTime") ||
      hasChanges("cookTime") ||
      hasChanges("totalTime") ||
      hasChanges("servings") ||
      hasChanges("difficulty") ||
      hasChanges("nutritionalInfo") ||
      hasChanges("sourceName") ||
      hasChanges("sourceUrl")
    );
  };

  return (
    <Modal
      isOpen={props.isOpen}
      onClose={props.onClose}
      title="Compare Versions"
      actions={
        <button type="button" class="btn" onClick={props.onClose}>
          Close
        </button>
      }
    >
      <Show when={props.loading}>
        <p class="loading-text">Loading versions...</p>
      </Show>

      <Show when={!props.loading && props.versionA && props.versionB}>
        <div class="version-compare-header">
          <div class="version-compare-label">
            <span class="version-compare-date">
              {formatDate(props.versionA!.updatedAt)}
            </span>
            <VersionSourceBadge source={props.versionA!.versionSource} />
          </div>
          <span class="version-compare-arrow">→</span>
          <div class="version-compare-label">
            <span class="version-compare-date">
              {formatDate(props.versionB!.updatedAt)}
            </span>
            <VersionSourceBadge source={props.versionB!.versionSource} />
          </div>
        </div>

        <div class="version-compare-content">
          <Show when={hasChanges("title")}>
            <DiffViewer
              label="Title"
              oldText={props.versionA!.title}
              newText={props.versionB!.title}
            />
          </Show>

          <Show when={hasChanges("description")}>
            <DiffViewer
              label="Description"
              oldText={props.versionA!.description || ""}
              newText={props.versionB!.description || ""}
            />
          </Show>

          <Show when={ingredientsChanged()}>
            <DiffViewer
              label="Ingredients"
              oldText={formatIngredients(props.versionA!.ingredients)}
              newText={formatIngredients(props.versionB!.ingredients)}
            />
          </Show>

          <Show when={hasChanges("instructions")}>
            <DiffViewer
              label="Instructions"
              oldText={props.versionA!.instructions}
              newText={props.versionB!.instructions}
            />
          </Show>

          <Show when={tagsChanged()}>
            <DiffViewer
              label="Tags"
              oldText={formatTags(props.versionA!.tags)}
              newText={formatTags(props.versionB!.tags)}
            />
          </Show>

          <Show when={hasChanges("notes")}>
            <DiffViewer
              label="Notes"
              oldText={props.versionA!.notes || ""}
              newText={props.versionB!.notes || ""}
            />
          </Show>

          <Show when={hasChanges("prepTime")}>
            <DiffViewer
              label="Prep Time"
              oldText={props.versionA!.prepTime || ""}
              newText={props.versionB!.prepTime || ""}
            />
          </Show>

          <Show when={hasChanges("cookTime")}>
            <DiffViewer
              label="Cook Time"
              oldText={props.versionA!.cookTime || ""}
              newText={props.versionB!.cookTime || ""}
            />
          </Show>

          <Show when={hasChanges("totalTime")}>
            <DiffViewer
              label="Total Time"
              oldText={props.versionA!.totalTime || ""}
              newText={props.versionB!.totalTime || ""}
            />
          </Show>

          <Show when={hasChanges("servings")}>
            <DiffViewer
              label="Servings"
              oldText={props.versionA!.servings || ""}
              newText={props.versionB!.servings || ""}
            />
          </Show>

          <Show when={hasChanges("difficulty")}>
            <DiffViewer
              label="Difficulty"
              oldText={props.versionA!.difficulty || ""}
              newText={props.versionB!.difficulty || ""}
            />
          </Show>

          <Show when={hasChanges("nutritionalInfo")}>
            <DiffViewer
              label="Nutritional Info"
              oldText={props.versionA!.nutritionalInfo || ""}
              newText={props.versionB!.nutritionalInfo || ""}
            />
          </Show>

          <Show when={hasChanges("sourceName")}>
            <DiffViewer
              label="Source Name"
              oldText={props.versionA!.sourceName || ""}
              newText={props.versionB!.sourceName || ""}
            />
          </Show>

          <Show when={hasChanges("sourceUrl")}>
            <DiffViewer
              label="Source URL"
              oldText={props.versionA!.sourceUrl || ""}
              newText={props.versionB!.sourceUrl || ""}
            />
          </Show>

          <Show when={!hasAnyChanges()}>
            <p class="no-changes">These versions are identical.</p>
          </Show>
        </div>
      </Show>
    </Modal>
  );
}
