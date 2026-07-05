import { createSignal, Show } from "solid-js";
import type { Accessor } from "solid-js";
import type { RecipeResponse, VersionSummary } from "ramekin-client";
import { useAuth } from "../context/AuthContext";
import { extractApiError } from "../utils/recipeFormHelpers";
import VersionCompareModal from "./VersionCompareModal";
import VersionHistoryPanel from "./VersionHistoryPanel";

interface RecipeVersioningSectionProps {
  recipeId: string;
  currentVersionId: Accessor<string | null>;
  onViewVersion: (versionId: string) => void;
  onRevertVersion: (version: VersionSummary) => void;
}

export default function RecipeVersioningSection(
  props: RecipeVersioningSectionProps,
) {
  const { getRecipesApi } = useAuth();
  const [compareLoading, setCompareLoading] = createSignal(false);
  const [compareVersions, setCompareVersions] = createSignal<
    [RecipeResponse, RecipeResponse] | null
  >(null);
  const [compareError, setCompareError] = createSignal<string | null>(null);

  const handleCompareVersions = async (versionIds: [string, string]) => {
    setCompareLoading(true);
    setCompareError(null);
    try {
      const [versionA, versionB] = await Promise.all([
        getRecipesApi().getRecipe({
          id: props.recipeId,
          versionId: versionIds[0],
        }),
        getRecipesApi().getRecipe({
          id: props.recipeId,
          versionId: versionIds[1],
        }),
      ]);

      if (versionA.updatedAt > versionB.updatedAt) {
        setCompareVersions([versionB, versionA]);
      } else {
        setCompareVersions([versionA, versionB]);
      }
    } catch (err) {
      const message = await extractApiError(
        err,
        "Failed to load versions for comparison",
      );
      setCompareError(message);
    } finally {
      setCompareLoading(false);
    }
  };

  const handleCompareClose = () => {
    setCompareVersions(null);
    setCompareError(null);
  };

  return (
    <>
      <Show when={props.currentVersionId()}>
        <VersionHistoryPanel
          recipeId={props.recipeId}
          currentVersionId={props.currentVersionId()!}
          onViewVersion={props.onViewVersion}
          onRevertVersion={props.onRevertVersion}
          onCompareVersions={handleCompareVersions}
        />
      </Show>

      <VersionCompareModal
        isOpen={() =>
          compareLoading() ||
          compareVersions() !== null ||
          compareError() !== null
        }
        onClose={handleCompareClose}
        loading={compareLoading()}
        error={compareError()}
        versionA={compareVersions()?.[0] ?? null}
        versionB={compareVersions()?.[1] ?? null}
      />
    </>
  );
}
