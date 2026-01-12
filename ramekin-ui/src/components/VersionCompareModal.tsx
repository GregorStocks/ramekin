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
  error: string | null;
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

  // Derive a tuple of both versions when both are present, for type-safe access
  const versionPair = (): [RecipeResponse, RecipeResponse] | null => {
    const a = props.versionA;
    const b = props.versionB;
    return a && b ? [a, b] : null;
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

      <Show when={props.error}>
        <p class="error">{props.error}</p>
      </Show>

      <Show when={!props.loading && !props.error && versionPair()}>
        {(pair) => {
          const [versionA, versionB] = pair();
          return (
            <>
              <div class="version-compare-header">
                <div class="version-compare-label">
                  <span class="version-compare-date">
                    {formatDate(versionA.updatedAt)}
                  </span>
                  <VersionSourceBadge source={versionA.versionSource} />
                </div>
                <span class="version-compare-arrow">→</span>
                <div class="version-compare-label">
                  <span class="version-compare-date">
                    {formatDate(versionB.updatedAt)}
                  </span>
                  <VersionSourceBadge source={versionB.versionSource} />
                </div>
              </div>

              <div class="version-compare-content">
                <Show when={hasChanges("title")}>
                  <DiffViewer
                    label="Title"
                    oldText={versionA.title}
                    newText={versionB.title}
                  />
                </Show>

                <Show when={hasChanges("description")}>
                  <DiffViewer
                    label="Description"
                    oldText={versionA.description || ""}
                    newText={versionB.description || ""}
                  />
                </Show>

                <Show when={ingredientsChanged()}>
                  <DiffViewer
                    label="Ingredients"
                    oldText={formatIngredients(versionA.ingredients)}
                    newText={formatIngredients(versionB.ingredients)}
                  />
                </Show>

                <Show when={hasChanges("instructions")}>
                  <DiffViewer
                    label="Instructions"
                    oldText={versionA.instructions}
                    newText={versionB.instructions}
                  />
                </Show>

                <Show when={tagsChanged()}>
                  <DiffViewer
                    label="Tags"
                    oldText={formatTags(versionA.tags)}
                    newText={formatTags(versionB.tags)}
                  />
                </Show>

                <Show when={hasChanges("notes")}>
                  <DiffViewer
                    label="Notes"
                    oldText={versionA.notes || ""}
                    newText={versionB.notes || ""}
                  />
                </Show>

                <Show when={hasChanges("prepTime")}>
                  <DiffViewer
                    label="Prep Time"
                    oldText={versionA.prepTime || ""}
                    newText={versionB.prepTime || ""}
                  />
                </Show>

                <Show when={hasChanges("cookTime")}>
                  <DiffViewer
                    label="Cook Time"
                    oldText={versionA.cookTime || ""}
                    newText={versionB.cookTime || ""}
                  />
                </Show>

                <Show when={hasChanges("totalTime")}>
                  <DiffViewer
                    label="Total Time"
                    oldText={versionA.totalTime || ""}
                    newText={versionB.totalTime || ""}
                  />
                </Show>

                <Show when={hasChanges("servings")}>
                  <DiffViewer
                    label="Servings"
                    oldText={versionA.servings || ""}
                    newText={versionB.servings || ""}
                  />
                </Show>

                <Show when={hasChanges("difficulty")}>
                  <DiffViewer
                    label="Difficulty"
                    oldText={versionA.difficulty || ""}
                    newText={versionB.difficulty || ""}
                  />
                </Show>

                <Show when={hasChanges("nutritionalInfo")}>
                  <DiffViewer
                    label="Nutritional Info"
                    oldText={versionA.nutritionalInfo || ""}
                    newText={versionB.nutritionalInfo || ""}
                  />
                </Show>

                <Show when={hasChanges("sourceName")}>
                  <DiffViewer
                    label="Source Name"
                    oldText={versionA.sourceName || ""}
                    newText={versionB.sourceName || ""}
                  />
                </Show>

                <Show when={hasChanges("sourceUrl")}>
                  <DiffViewer
                    label="Source URL"
                    oldText={versionA.sourceUrl || ""}
                    newText={versionB.sourceUrl || ""}
                  />
                </Show>

                <Show when={!hasAnyChanges()}>
                  <p class="no-changes">These versions are identical.</p>
                </Show>
              </div>
            </>
          );
        }}
      </Show>
    </Modal>
  );
}
