import {
  createSignal,
  createEffect,
  createMemo,
  Show,
  For,
  onMount,
  onCleanup,
} from "solid-js";
import { useParams, A, useNavigate, useSearchParams } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import StarRating from "../components/StarRating";
import Modal from "../components/Modal";
import EnrichPreviewModal from "../components/EnrichPreviewModal";
import AddToShoppingListModal from "../components/AddToShoppingListModal";
import AddToMealPlanModal from "../components/AddToMealPlanModal";
import RecipeVersioningSection from "../components/RecipeVersioningSection";
import { useRecipeAiActions } from "../hooks/useRecipeAiActions";
import {
  extractApiError,
  parseApiError,
  groupIngredientsBySection,
} from "../utils/recipeFormHelpers";
import { parseTag } from "../utils/tagHierarchy";
import { usePageTitle } from "../utils/pageTitle";
import { scaleAmount } from "../utils/scaleAmount";
import { formatIngredientParts } from "../utils/ingredientFormatting";
import { AI_ENRICHMENTS } from "../utils/aiEnrichments";
import { pollScrapeJob } from "../utils/pollScrapeJob";
import type { RecipeResponse, VersionSummary } from "ramekin-client";
import { ErrorCode } from "ramekin-client";

function PhotoImage(props: { photoId: string; token: string; alt: string }) {
  const { authedFetch } = useAuth();
  const [src, setSrc] = createSignal<string | null>(null);

  onMount(async () => {
    const response = await authedFetch(`/api/photos/${props.photoId}`, {
      headers: { Authorization: `Bearer ${props.token}` },
    });
    if (response.ok) {
      const blob = await response.blob();
      setSrc(URL.createObjectURL(blob));
    }
  });

  onCleanup(() => {
    const url = src();
    if (url) URL.revokeObjectURL(url);
  });

  return (
    <Show when={src()}>
      <img src={src()!} alt={props.alt} class="recipe-photo" />
    </Show>
  );
}

export default function ViewRecipePage() {
  const params = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const { getRecipesApi, getEnrichApi, getScrapeApi, token } = useAuth();

  // Check if we're in "random browsing" mode
  const randomQuery = () =>
    typeof searchParams.randomQ === "string" ? searchParams.randomQ : null;
  const isRandomMode = () => randomQuery() !== null;

  // Get version_id from URL params
  const versionId = () =>
    typeof searchParams.version_id === "string"
      ? searchParams.version_id
      : null;

  const SCALE_PRESETS = [
    { value: 0.25, label: "¼×" },
    { value: 0.5, label: "½×" },
    { value: 1, label: "1×" },
    { value: 2, label: "2×" },
    { value: 3, label: "3×" },
  ];

  const scale = () => {
    const raw = searchParams.scale;
    const v = typeof raw === "string" ? Number(raw) : NaN;
    return Number.isFinite(v) && v > 0 ? v : 1;
  };

  const setScale = (v: number) => {
    if (!Number.isFinite(v) || v <= 0) return;
    setSearchParams({ scale: v === 1 ? undefined : String(v) });
  };

  const [customScaleInput, setCustomScaleInput] = createSignal("");
  const applyCustomScale = () => {
    const v = Number(customScaleInput());
    if (Number.isFinite(v) && v > 0) {
      setScale(v);
    }
  };

  const formatScaleLabel = (v: number): string => {
    const pretty: Record<string, string> = {
      "0.25": "¼",
      "0.5": "½",
      "0.3333333333333333": "⅓",
      "0.6666666666666666": "⅔",
    };
    const key = String(v);
    if (key in pretty) return `${pretty[key]}×`;
    const trimmed = v.toFixed(2).replace(/\.?0+$/, "");
    return `${trimmed}×`;
  };

  const [recipe, setRecipe] = createSignal<RecipeResponse | null>(null);
  usePageTitle(() => recipe()?.title);
  const [currentVersionId, setCurrentVersionId] = createSignal<string | null>(
    null,
  );
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [deleting, setDeleting] = createSignal(false);
  const [showDeleteModal, setShowDeleteModal] = createSignal(false);
  // Revert state
  const [revertVersion, setRevertVersion] = createSignal<VersionSummary | null>(
    null,
  );
  const [reverting, setReverting] = createSignal(false);

  // Rescrape state
  const [rescraping, setRescraping] = createSignal(false);

  // Shopping list modal state
  const [showShoppingListModal, setShowShoppingListModal] = createSignal(false);

  // Meal plan modal state
  const [showMealPlanModal, setShowMealPlanModal] = createSignal(false);

  const loadRecipe = async () => {
    setLoading(true);
    setError(null);
    try {
      const vid = versionId();
      const response = await getRecipesApi().getRecipe({
        id: params.id,
        versionId: vid ?? undefined,
      });
      setRecipe(response);
      if (!vid) {
        setCurrentVersionId(response.versionId);
      } else if (!currentVersionId()) {
        // Initial load with a version param — also fetch the latest version ID
        // so we can show the "viewing historical version" banner.
        loadCurrentVersionId();
      }
    } catch (err) {
      const parsed = await parseApiError(err, "Failed to load recipe");
      setError(
        parsed.code === ErrorCode.NotFound
          ? "Recipe not found"
          : "Failed to load recipe",
      );
    } finally {
      setLoading(false);
    }
  };

  // Load current version ID on mount (before potentially loading a specific version)
  const loadCurrentVersionId = async () => {
    try {
      const response = await getRecipesApi().getRecipe({ id: params.id });
      setCurrentVersionId(response.versionId);
    } catch {
      // Ignore - will be handled by main loadRecipe
    }
  };

  const handleDelete = () => {
    setShowDeleteModal(true);
  };

  const goBackToCookbook = () => {
    if (window.history.length > 1) {
      navigate(-1);
    } else {
      navigate("/");
    }
  };

  const handleDeleteConfirm = async () => {
    setDeleting(true);
    try {
      await getRecipesApi().deleteRecipe({ id: params.id });
      goBackToCookbook();
    } catch (err) {
      const message = await extractApiError(err, "Failed to delete recipe");
      setError(message);
      setShowDeleteModal(false);
      setDeleting(false);
    }
  };

  const goToNextRandom = async () => {
    const q = randomQuery();
    if (q === null) return;
    try {
      const response = await getRecipesApi().listRecipes({
        q: q || undefined,
        limit: 1,
        sortBy: "random",
      });
      if (response.recipes.length > 0) {
        navigate(
          `/recipes/${response.recipes[0].id}?randomQ=${encodeURIComponent(q)}`,
        );
      }
    } catch (err) {
      const message = await extractApiError(
        err,
        "Failed to load next random recipe",
      );
      setError(message);
    }
  };

  // Version viewing handlers
  const handleViewVersion = (vid: string) => {
    setSearchParams({ version_id: vid });
  };

  const handleViewCurrent = () => {
    setSearchParams({ version_id: undefined });
  };

  // Check if viewing a historical version
  const isViewingHistoricalVersion = () => {
    const vid = versionId();
    const currentVid = currentVersionId();
    return vid !== null && currentVid !== null && vid !== currentVid;
  };

  // Revert handlers
  const handleRevertClick = (version: VersionSummary) => {
    setRevertVersion(version);
  };

  const handleRevertConfirm = async () => {
    const version = revertVersion();
    if (!version) return;

    setReverting(true);
    try {
      // Fetch the full recipe content at that version
      const oldRecipe = await getRecipesApi().getRecipe({
        id: params.id,
        versionId: version.id,
      });

      // Update the recipe with that content (creates new version)
      await getRecipesApi().updateRecipe({
        id: params.id,
        updateRecipeRequest: {
          title: oldRecipe.title,
          description: oldRecipe.description,
          instructions: oldRecipe.instructions,
          ingredients: oldRecipe.ingredients,
          tags: oldRecipe.tags,
          prepTime: oldRecipe.prepTime,
          cookTime: oldRecipe.cookTime,
          totalTime: oldRecipe.totalTime,
          servings: oldRecipe.servings,
          difficulty: oldRecipe.difficulty,
          rating: oldRecipe.rating,
          notes: oldRecipe.notes,
          nutritionalInfo: oldRecipe.nutritionalInfo,
          sourceName: oldRecipe.sourceName,
          sourceUrl: oldRecipe.sourceUrl,
        },
      });

      // Clear version param — the createEffect tracking versionId() will
      // automatically reload the recipe with the new (null) version.
      setSearchParams({ version_id: undefined });
      setRevertVersion(null);
    } catch (err) {
      const message = await extractApiError(
        err,
        "Failed to revert to this version",
      );
      setError(message);
    } finally {
      setReverting(false);
    }
  };

  const handleRevertCancel = () => {
    setRevertVersion(null);
  };

  const recipeAiActions = useRecipeAiActions({
    recipeId: () => params.id,
    recipe,
    getRecipesApi,
    getEnrichApi,
    loadRecipe,
    clearHistoricalVersion: () => setSearchParams({ version_id: undefined }),
    setError,
  });

  // Rescrape handler
  const handleRescrape = async () => {
    const r = recipe();
    if (!r || !r.sourceUrl) return;

    setRescraping(true);
    setError(null);
    try {
      // Start the rescrape job
      const response = await getRecipesApi().rescrape({ id: params.id });
      const jobId = response.jobId;

      const result = await pollScrapeJob(getScrapeApi(), jobId);
      if (result.status === "completed") {
        await loadRecipe();
      } else if (result.status === "failed") {
        setError(`Rescrape failed: ${result.error}`);
      } else {
        setError("Rescrape timed out");
      }
    } catch (err) {
      const message = await extractApiError(err, "Failed to rescrape recipe");
      setError(message);
    } finally {
      setRescraping(false);
    }
  };

  const formatDate = (date: Date) => {
    return new Intl.DateTimeFormat("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
      hour: "numeric",
      minute: "2-digit",
    }).format(date);
  };

  const openMealPlanModal = () => {
    setShowMealPlanModal(true);
  };

  const closeMealPlanModal = () => {
    setShowMealPlanModal(false);
  };

  // Reload when version_id changes
  createEffect(() => {
    // Read versionId to track it as a dependency
    versionId();
    loadRecipe();
  });

  return (
    <div class="view-recipe-page">
      <Show when={loading()}>
        <p class="loading">Loading recipe...</p>
      </Show>

      <Show when={error()}>
        <div class="error-state">
          <p class="error">{error()}</p>
          <button type="button" class="btn" onClick={goBackToCookbook}>
            Back to Cookbook
          </button>
        </div>
      </Show>

      <Show when={recipe()}>
        {(r) => (
          <>
            <div class="recipe-top-bar">
              <div class="recipe-nav-links">
                <button
                  type="button"
                  class="back-link"
                  onClick={goBackToCookbook}
                >
                  &larr; Back
                </button>
                <Show when={isRandomMode()}>
                  <button
                    type="button"
                    class="btn btn-small"
                    onClick={goToNextRandom}
                  >
                    Next Random &rarr;
                  </button>
                </Show>
              </div>
              <div class="recipe-actions">
                <Show when={r().sourceUrl}>
                  <button
                    type="button"
                    class="btn"
                    onClick={handleRescrape}
                    disabled={rescraping() || isViewingHistoricalVersion()}
                  >
                    {rescraping() ? "Rescraping..." : "Rescrape"}
                  </button>
                </Show>
                <button
                  type="button"
                  class="btn"
                  onClick={recipeAiActions.handleNormalizeTitle}
                  disabled={
                    recipeAiActions.normalizingTitle() ||
                    isViewingHistoricalVersion() ||
                    loading()
                  }
                >
                  {recipeAiActions.normalizingTitle()
                    ? "Renaming..."
                    : AI_ENRICHMENTS.normalizeTitle.individualLabel}
                </button>
                <button
                  type="button"
                  class="btn"
                  onClick={recipeAiActions.handleGenerateDescription}
                  disabled={
                    recipeAiActions.generatingDescription() ||
                    isViewingHistoricalVersion() ||
                    loading()
                  }
                >
                  {recipeAiActions.generatingDescription()
                    ? "Generating..."
                    : AI_ENRICHMENTS.generateDescription.individualLabel}
                </button>
                <button
                  type="button"
                  class="btn"
                  onClick={recipeAiActions.handleEnrich}
                  disabled={
                    recipeAiActions.enriching() || isViewingHistoricalVersion()
                  }
                >
                  {recipeAiActions.enriching()
                    ? "Enriching..."
                    : AI_ENRICHMENTS.enrichRecipe.individualLabel}
                </button>
                <button
                  type="button"
                  class="btn"
                  onClick={() =>
                    recipeAiActions.setShowCustomEnrichInput(
                      !recipeAiActions.showCustomEnrichInput(),
                    )
                  }
                  disabled={
                    recipeAiActions.enriching() || isViewingHistoricalVersion()
                  }
                >
                  {AI_ENRICHMENTS.customEnrich.individualLabel}
                </button>
                <button
                  type="button"
                  class="btn"
                  onClick={recipeAiActions.handleGeneratePhoto}
                  disabled={
                    recipeAiActions.generatingPhoto() ||
                    isViewingHistoricalVersion()
                  }
                >
                  {recipeAiActions.generatingPhoto()
                    ? "Generating Photo..."
                    : AI_ENRICHMENTS.generatePhoto.individualLabel}
                </button>
                <button
                  type="button"
                  class="btn"
                  onClick={() => setShowShoppingListModal(true)}
                  disabled={isViewingHistoricalVersion()}
                >
                  Add to Shopping List
                </button>
                <button
                  type="button"
                  class="btn"
                  onClick={openMealPlanModal}
                  disabled={isViewingHistoricalVersion()}
                >
                  Add to Meal Plan
                </button>
                <A href={`/recipes/${params.id}/edit`} class="btn btn-primary">
                  Edit
                </A>
                <button
                  class="btn btn-danger-outline"
                  onClick={handleDelete}
                  disabled={deleting()}
                >
                  {deleting() ? "Deleting..." : "Delete"}
                </button>
              </div>
              <Show when={recipeAiActions.showCustomEnrichInput()}>
                <div
                  class="custom-enrich-input"
                  style={{ display: "flex", gap: "8px", "margin-top": "8px" }}
                >
                  <input
                    type="text"
                    placeholder="e.g., make this vegan, double the servings..."
                    value={recipeAiActions.customInstruction()}
                    onInput={(e) =>
                      recipeAiActions.setCustomInstruction(
                        e.currentTarget.value,
                      )
                    }
                    onKeyDown={(e) => {
                      if (e.key === "Enter")
                        recipeAiActions.handleCustomEnrich();
                    }}
                    disabled={recipeAiActions.enriching()}
                    style={{ flex: "1" }}
                  />
                  <button
                    type="button"
                    class="btn btn-primary"
                    onClick={recipeAiActions.handleCustomEnrich}
                    disabled={
                      recipeAiActions.enriching() ||
                      !recipeAiActions.customInstruction().trim()
                    }
                  >
                    {recipeAiActions.enriching() ? "Customizing..." : "Go"}
                  </button>
                </div>
              </Show>
            </div>

            {/* Historical version banner */}
            <Show when={isViewingHistoricalVersion()}>
              <div class="version-banner">
                <span>
                  You are viewing a version from {formatDate(r().updatedAt)}
                </span>
                <div class="version-banner-actions">
                  <button
                    type="button"
                    class="btn btn-small"
                    onClick={handleViewCurrent}
                  >
                    View Current
                  </button>
                  <button
                    type="button"
                    class="btn btn-small btn-primary"
                    onClick={() =>
                      handleRevertClick({
                        id: r().versionId,
                        title: r().title,
                        createdAt: r().updatedAt,
                        isCurrent: false,
                        versionSource: r().versionSource,
                      })
                    }
                  >
                    Revert to This Version
                  </button>
                </div>
              </div>
            </Show>

            <RecipeVersioningSection
              recipeId={params.id}
              currentVersionId={currentVersionId}
              onViewVersion={handleViewVersion}
              onRevertVersion={handleRevertClick}
            />

            <div class="recipe-header-compact">
              <h2>{r().title}</h2>
              <Show when={r().tags && r().tags.length > 0}>
                <div class="recipe-tags">
                  <For each={r().tags}>
                    {(tag) => {
                      const parsed = parseTag(tag);
                      return (
                        <span class="tag">
                          <Show when={parsed.namespace}>
                            <span class="tag-chip-ns">{parsed.namespace}:</span>
                          </Show>
                          {parsed.value}
                        </span>
                      );
                    }}
                  </For>
                </div>
              </Show>
              <Show when={r().description}>
                <p class="recipe-description">{r().description}</p>
              </Show>
              <Show when={r().sourceUrl || r().sourceName}>
                <div class="recipe-source-inline">
                  <Show
                    when={r().sourceUrl}
                    fallback={<span>{r().sourceName}</span>}
                  >
                    <a
                      href={r().sourceUrl!}
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      {r().sourceName || "Source"}
                    </a>
                  </Show>
                </div>
              </Show>
              <Show
                when={
                  r().servings ||
                  r().prepTime ||
                  r().cookTime ||
                  r().totalTime ||
                  r().rating ||
                  r().difficulty
                }
              >
                <div class="recipe-metadata">
                  <Show when={r().servings}>
                    <div class="recipe-metadata-item">
                      <span class="label">Serves:</span>
                      <span class="value">
                        {scaleAmount(r().servings, scale())}
                      </span>
                      <Show when={scale() !== 1}>
                        <span class="scale-badge">
                          scaled {formatScaleLabel(scale())}
                        </span>
                      </Show>
                    </div>
                  </Show>
                  <Show when={r().prepTime}>
                    <div class="recipe-metadata-item">
                      <span class="label">Prep:</span>
                      <span class="value">{r().prepTime}</span>
                    </div>
                  </Show>
                  <Show when={r().cookTime}>
                    <div class="recipe-metadata-item">
                      <span class="label">Cook:</span>
                      <span class="value">{r().cookTime}</span>
                    </div>
                  </Show>
                  <Show when={r().totalTime}>
                    <div class="recipe-metadata-item">
                      <span class="label">Total:</span>
                      <span class="value">{r().totalTime}</span>
                    </div>
                  </Show>
                  <Show when={r().rating}>
                    <div class="recipe-metadata-item">
                      <StarRating rating={r().rating ?? null} readonly />
                    </div>
                  </Show>
                  <Show when={r().difficulty}>
                    <div class="recipe-metadata-item">
                      <span class="difficulty-badge">{r().difficulty}</span>
                    </div>
                  </Show>
                </div>
              </Show>
            </div>

            <div class="recipe-layout">
              <Show when={r().ingredients && r().ingredients.length > 0}>
                <div class="recipe-left">
                  <section class="recipe-section">
                    <h3>
                      Ingredients
                      <Show when={scale() !== 1}>
                        {" "}
                        <span class="scale-badge">
                          scaled {formatScaleLabel(scale())}
                        </span>
                      </Show>
                    </h3>
                    <div class="scale-controls">
                      <span class="scale-label">Scale:</span>
                      <For each={SCALE_PRESETS}>
                        {(preset) => (
                          <button
                            type="button"
                            class={
                              scale() === preset.value
                                ? "scale-preset active"
                                : "scale-preset"
                            }
                            onClick={() => {
                              setCustomScaleInput("");
                              setScale(preset.value);
                            }}
                          >
                            {preset.label}
                          </button>
                        )}
                      </For>
                      <input
                        type="number"
                        step="0.25"
                        min="0"
                        class="scale-custom-input"
                        placeholder="Custom"
                        value={customScaleInput()}
                        onInput={(e) =>
                          setCustomScaleInput(e.currentTarget.value)
                        }
                        onKeyDown={(e) => {
                          if (e.key === "Enter") applyCustomScale();
                        }}
                        onBlur={applyCustomScale}
                      />
                    </div>
                    <For
                      each={groupIngredientsBySection(r().ingredients ?? [])}
                    >
                      {(group) => (
                        <>
                          <Show when={group.section}>
                            <h4 class="ingredient-section-header">
                              {group.section}
                            </h4>
                          </Show>
                          <ul class="ingredients-list">
                            <For each={group.ingredients}>
                              {(ing) => {
                                const parts = createMemo(() =>
                                  formatIngredientParts(ing, {
                                    scale: scale(),
                                    includeAlternatives: true,
                                    includeNote: true,
                                  }),
                                );
                                return (
                                  <li>
                                    <Show when={parts().amount}>
                                      <span class="amount">
                                        {parts().amount}
                                      </span>{" "}
                                    </Show>
                                    <Show when={parts().unit}>
                                      <span class="unit">
                                        {parts().unit}
                                      </span>{" "}
                                    </Show>
                                    <Show when={parts().alternatives}>
                                      <span class="alt-measurement">
                                        ({parts().alternatives}){" "}
                                      </span>
                                    </Show>
                                    <span class="item">{parts().item}</span>
                                    <Show when={parts().note}>
                                      <span class="note">
                                        {" "}
                                        ({parts().note})
                                      </span>
                                    </Show>
                                  </li>
                                );
                              }}
                            </For>
                          </ul>
                        </>
                      )}
                    </For>
                  </section>
                </div>
              </Show>

              <div class="recipe-right">
                <Show when={r().photoIds && r().photoIds.length > 0}>
                  <div class="recipe-photos">
                    <For each={r().photoIds}>
                      {(photoId) => (
                        <PhotoImage
                          photoId={photoId}
                          token={token() ?? ""}
                          alt="Recipe photo"
                        />
                      )}
                    </For>
                  </div>
                </Show>
                <section class="recipe-section">
                  <h3>Instructions</h3>
                  <div class="instructions">{r().instructions}</div>
                </section>
                <Show when={r().notes}>
                  <section class="recipe-section">
                    <h3>Notes</h3>
                    <div class="recipe-notes">{r().notes}</div>
                  </section>
                </Show>
                <Show when={r().nutritionalInfo}>
                  <section class="recipe-section">
                    <h3>Nutritional Info</h3>
                    <div class="recipe-notes">{r().nutritionalInfo}</div>
                  </section>
                </Show>
              </div>
            </div>

            {/* Revert Confirmation Modal */}
            <Modal
              isOpen={() => revertVersion() !== null}
              onClose={handleRevertCancel}
              title="Revert to this version?"
              actions={
                <>
                  <button
                    type="button"
                    class="btn"
                    onClick={handleRevertCancel}
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    class="btn btn-primary"
                    onClick={handleRevertConfirm}
                    disabled={reverting()}
                  >
                    {reverting() ? "Reverting..." : "Revert"}
                  </button>
                </>
              }
            >
              <p>
                This will create a new version with the content from{" "}
                <strong>
                  {revertVersion() && formatDate(revertVersion()!.createdAt)}
                </strong>
                .
              </p>
              <p>The current version will be preserved in history.</p>
            </Modal>

            {/* Enrich Preview Modal */}
            <Show when={recipeAiActions.enrichedContent() && recipe()}>
              <EnrichPreviewModal
                isOpen={() => recipeAiActions.enrichedContent() !== null}
                onClose={recipeAiActions.handleEnrichClose}
                currentRecipe={recipe()!}
                enrichedContent={recipeAiActions.enrichedContent()!}
                onApply={recipeAiActions.handleApplyEnrichment}
                applying={recipeAiActions.applyingEnrichment()}
              />
            </Show>

            {/* Add to Shopping List Modal */}
            <AddToShoppingListModal
              isOpen={showShoppingListModal}
              onClose={() => setShowShoppingListModal(false)}
              recipe={r()}
              scale={scale}
            />

            {/* Delete Confirmation Modal */}
            <Modal
              isOpen={showDeleteModal}
              onClose={() => setShowDeleteModal(false)}
              title="Delete Recipe"
              actions={
                <>
                  <button
                    type="button"
                    class="btn"
                    onClick={() => setShowDeleteModal(false)}
                    disabled={deleting()}
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    class="btn btn-danger"
                    onClick={handleDeleteConfirm}
                    disabled={deleting()}
                  >
                    {deleting() ? "Deleting..." : "Delete"}
                  </button>
                </>
              }
            >
              <p>
                Are you sure you want to delete this recipe? This cannot be
                undone.
              </p>
            </Modal>

            <AddToMealPlanModal
              isOpen={showMealPlanModal}
              onClose={closeMealPlanModal}
              recipeId={params.id}
            />
          </>
        )}
      </Show>
    </div>
  );
}
