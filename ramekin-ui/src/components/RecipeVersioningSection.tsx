import { createSignal, Show } from "solid-js";
import type { Accessor } from "solid-js";
import type { RecipeResponse, VersionSummary } from "ramekin-client";
import { useAuth } from "../context/AuthContext";
import { extractApiError } from "../utils/recipeFormHelpers";
import { createRequestTracker } from "../utils/requestTracker";
import VersionCompareModal from "./VersionCompareModal";
import VersionHistoryPanel from "./VersionHistoryPanel";

interface RecipeVersioningSectionProps {
  recipeId: string;
  currentVersionId: Accessor<string | null>;
  onViewVersion: (versionId: string) => void;
  onRevertVersion: (version: VersionSummary) => void;
}

/** The compare modal: closed, or open on a load, its result, or its error. */
type CompareState =
  | { kind: "closed" }
  | { kind: "loading" }
  | { kind: "loaded"; versions: [RecipeResponse, RecipeResponse] }
  | { kind: "error"; message: string };

export default function RecipeVersioningSection(
  props: RecipeVersioningSectionProps,
) {
  const { getRecipesApi } = useAuth();
  const [compare, setCompare] = createSignal<CompareState>({ kind: "closed" });
  // Closing the modal or starting another comparison drops a load in flight.
  const compareRequests = createRequestTracker();

  const compareVersions = () => {
    const state = compare();
    return state.kind === "loaded" ? state.versions : null;
  };
  const compareError = () => {
    const state = compare();
    return state.kind === "error" ? state.message : null;
  };

  const handleCompareVersions = async (versionIds: [string, string]) => {
    const request = compareRequests.start();
    setCompare({ kind: "loading" });
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
      if (!compareRequests.isCurrent(request)) return;

      setCompare({
        kind: "loaded",
        versions:
          versionA.updatedAt > versionB.updatedAt
            ? [versionB, versionA]
            : [versionA, versionB],
      });
    } catch (err) {
      const message = await extractApiError(
        err,
        "Failed to load versions for comparison",
      );
      if (!compareRequests.isCurrent(request)) return;
      setCompare({ kind: "error", message });
    }
  };

  const handleCompareClose = () => {
    compareRequests.invalidate();
    setCompare({ kind: "closed" });
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
        isOpen={() => compare().kind !== "closed"}
        onClose={handleCompareClose}
        loading={compare().kind === "loading"}
        error={compareError()}
        versionA={compareVersions()?.[0] ?? null}
        versionB={compareVersions()?.[1] ?? null}
      />
    </>
  );
}
