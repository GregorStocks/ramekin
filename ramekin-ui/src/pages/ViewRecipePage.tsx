import {
  createSignal,
  createEffect,
  Show,
  For,
  onMount,
  onCleanup,
} from "solid-js";
import { useParams, A, useNavigate, useSearchParams } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import StarRating from "../components/StarRating";
import Modal from "../components/Modal";
import VersionHistoryPanel from "../components/VersionHistoryPanel";
import EnrichPreviewModal from "../components/EnrichPreviewModal";
import VersionCompareModal from "../components/VersionCompareModal";
import AddToShoppingListModal from "../components/AddToShoppingListModal";
import { extractApiError } from "../utils/recipeFormHelpers";
import type {
  RecipeResponse,
  RecipeContent,
  VersionSummary,
  Ingredient,
  MealType,
} from "ramekin-client";

/** Group ingredients by contiguous sections (preserving order). */
function groupIngredientsBySection(
  ingredients: Ingredient[],
): Array<{ section: string | null; ingredients: Ingredient[] }> {
  const groups: Array<{ section: string | null; ingredients: Ingredient[] }> =
    [];

  for (const ing of ingredients) {
    const section = ing.section ?? null;
    const lastGroup = groups[groups.length - 1];

    // If this ingredient has the same section as the last group, add to it
    if (lastGroup && lastGroup.section === section) {
      lastGroup.ingredients.push(ing);
    } else {
      // Start a new group
      groups.push({ section, ingredients: [ing] });
    }
  }

  return groups;
}

function PhotoImage(props: { photoId: string; token: string; alt: string }) {
  const [src, setSrc] = createSignal<string | null>(null);

  onMount(async () => {
    const response = await fetch(`/api/photos/${props.photoId}`, {
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

const MEAL_TYPES: MealType[] = ["breakfast", "lunch", "dinner", "snack"];
const MEAL_TYPE_LABELS: Record<MealType, string> = {
  breakfast: "Breakfast",
  lunch: "Lunch",
  dinner: "Dinner",
  snack: "Snack",
};

function getTodayString(): string {
  const d = new Date();
  const year = d.getFullYear();
  const month = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function toApiDate(dateStr: string): Date {
  const [year, month, day] = dateStr.split("-").map(Number);
  return new Date(Date.UTC(year, month - 1, day));
}

export default function ViewRecipePage() {
  const params = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const { getRecipesApi, getEnrichApi, getScrapeApi, getMealPlansApi, token } =
    useAuth();

  // Check if we're in "random browsing" mode
  const randomQuery = () =>
    typeof searchParams.randomQ === "string" ? searchParams.randomQ : null;
  const isRandomMode = () => randomQuery() !== null;

  // Get version_id from URL params
  const versionId = () =>
    typeof searchParams.version_id === "string"
      ? searchParams.version_id
      : null;

  const [recipe, setRecipe] = createSignal<RecipeResponse | null>(null);
  const [currentVersionId, setCurrentVersionId] = createSignal<string | null>(
    null,
  );
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [deleting, setDeleting] = createSignal(false);
  // Revert state
  const [revertVersion, setRevertVersion] = createSignal<VersionSummary | null>(
    null,
  );
  const [reverting, setReverting] = createSignal(false);

  // Enrich state
  const [enriching, setEnriching] = createSignal(false);
  const [enrichedContent, setEnrichedContent] =
    createSignal<RecipeContent | null>(null);
  const [applyingEnrichment, setApplyingEnrichment] = createSignal(false);

  // Compare state
  const [compareLoading, setCompareLoading] = createSignal(false);
  const [compareVersions, setCompareVersions] = createSignal<
    [RecipeResponse, RecipeResponse] | null
  >(null);
  const [compareError, setCompareError] = createSignal<string | null>(null);

  // Rescrape state
  const [rescraping, setRescraping] = createSignal(false);

  // Shopping list modal state
  const [showShoppingListModal, setShowShoppingListModal] = createSignal(false);

  // Meal plan modal state
  const [showMealPlanModal, setShowMealPlanModal] = createSignal(false);
  const [mealPlanDate, setMealPlanDate] = createSignal(getTodayString());
  const [mealPlanMealType, setMealPlanMealType] =
    createSignal<MealType>("dinner");
  const [addingToMealPlan, setAddingToMealPlan] = createSignal(false);
  const [mealPlanError, setMealPlanError] = createSignal<string | null>(null);
  const [mealPlanSuccess, setMealPlanSuccess] = createSignal(false);

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
      // Store the current version ID when not viewing a specific version
      if (!vid) {
        setCurrentVersionId(response.versionId);
      }
    } catch (err) {
      if (err instanceof Response && err.status === 404) {
        setError("Recipe not found");
      } else {
        setError("Failed to load recipe");
      }
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

  const handleDelete = async () => {
    if (!confirm("Are you sure you want to delete this recipe?")) {
      return;
    }

    setDeleting(true);
    try {
      await getRecipesApi().deleteRecipe({ id: params.id });
      navigate("/");
    } catch (err) {
      setError("Failed to delete recipe");
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
    } catch {
      // Ignore errors
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

      // Clear version param and reload
      setSearchParams({ version_id: undefined });
      setRevertVersion(null);
      await loadRecipe();
      await loadCurrentVersionId();
    } catch (err) {
      setError("Failed to revert to this version");
    } finally {
      setReverting(false);
    }
  };

  const handleRevertCancel = () => {
    setRevertVersion(null);
  };

  // Enrich handlers
  const handleEnrich = async () => {
    const r = recipe();
    if (!r) return;

    setEnriching(true);
    setError(null);
    try {
      const enriched = await getEnrichApi().enrichRecipe({
        recipeContent: {
          title: r.title,
          description: r.description,
          instructions: r.instructions,
          ingredients: r.ingredients,
          tags: r.tags,
          prepTime: r.prepTime,
          cookTime: r.cookTime,
          totalTime: r.totalTime,
          servings: r.servings,
          difficulty: r.difficulty,
          notes: r.notes,
          nutritionalInfo: r.nutritionalInfo,
          sourceName: r.sourceName,
          sourceUrl: r.sourceUrl,
        },
      });
      setEnrichedContent(enriched);
    } catch (err) {
      setError("Failed to enrich recipe");
    } finally {
      setEnriching(false);
    }
  };

  const handleApplyEnrichment = async () => {
    const enriched = enrichedContent();
    if (!enriched) return;

    setApplyingEnrichment(true);
    try {
      await getRecipesApi().updateRecipe({
        id: params.id,
        updateRecipeRequest: {
          title: enriched.title,
          description: enriched.description,
          instructions: enriched.instructions,
          ingredients: enriched.ingredients,
          tags: enriched.tags,
          prepTime: enriched.prepTime,
          cookTime: enriched.cookTime,
          totalTime: enriched.totalTime,
          servings: enriched.servings,
          difficulty: enriched.difficulty,
          notes: enriched.notes,
          nutritionalInfo: enriched.nutritionalInfo,
          sourceName: enriched.sourceName,
          sourceUrl: enriched.sourceUrl,
        },
      });
      setEnrichedContent(null);
      await loadRecipe();
      await loadCurrentVersionId();
    } catch (err) {
      setError("Failed to apply enrichment");
    } finally {
      setApplyingEnrichment(false);
    }
  };

  const handleEnrichClose = () => {
    setEnrichedContent(null);
  };

  // Compare handlers
  const handleCompareVersions = async (versionIds: [string, string]) => {
    setCompareLoading(true);
    setCompareError(null);
    try {
      const [versionA, versionB] = await Promise.all([
        getRecipesApi().getRecipe({
          id: params.id,
          versionId: versionIds[0],
        }),
        getRecipesApi().getRecipe({
          id: params.id,
          versionId: versionIds[1],
        }),
      ]);
      // Order by date (older first)
      if (versionA.updatedAt > versionB.updatedAt) {
        setCompareVersions([versionB, versionA]);
      } else {
        setCompareVersions([versionA, versionB]);
      }
    } catch (err) {
      setCompareError("Failed to load versions for comparison");
    } finally {
      setCompareLoading(false);
    }
  };

  const handleCompareClose = () => {
    setCompareVersions(null);
    setCompareError(null);
  };

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

      // Poll for completion
      const poll = async (): Promise<void> => {
        const job = await getScrapeApi().getScrape({ id: jobId });

        if (job.status === "completed") {
          await loadRecipe();
          await loadCurrentVersionId();
          setRescraping(false);
        } else if (job.status === "failed") {
          setError(`Rescrape failed: ${job.error || "Unknown error"}`);
          setRescraping(false);
        } else {
          // Continue polling
          await new Promise((resolve) => setTimeout(resolve, 500));
          await poll();
        }
      };

      await poll();
    } catch (err) {
      setError("Failed to rescrape recipe");
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

  // Meal plan handlers
  const openMealPlanModal = () => {
    setMealPlanDate(getTodayString());
    setMealPlanMealType("dinner");
    setMealPlanError(null);
    setMealPlanSuccess(false);
    setShowMealPlanModal(true);
  };

  const closeMealPlanModal = () => {
    setShowMealPlanModal(false);
    setMealPlanError(null);
    setMealPlanSuccess(false);
  };

  const handleAddToMealPlan = async () => {
    setAddingToMealPlan(true);
    setMealPlanError(null);
    try {
      await getMealPlansApi().createMealPlan({
        createMealPlanRequest: {
          recipeId: params.id,
          mealDate: toApiDate(mealPlanDate()),
          mealType: mealPlanMealType(),
        },
      });
      setMealPlanSuccess(true);
      setTimeout(() => {
        closeMealPlanModal();
      }, 1500);
    } catch (err) {
      if (err instanceof Response && err.status === 409) {
        setMealPlanError(
          `This recipe is already scheduled for ${MEAL_TYPE_LABELS[mealPlanMealType()].toLowerCase()} on this date`,
        );
      } else {
        const message = await extractApiError(
          err,
          "Failed to add to meal plan",
        );
        setMealPlanError(message);
      }
    } finally {
      setAddingToMealPlan(false);
    }
  };

  onMount(() => {
    loadCurrentVersionId();
    loadRecipe();
  });

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
          <A href="/" class="btn">
            Back to Cookbook
          </A>
        </div>
      </Show>

      <Show when={recipe()}>
        {(r) => (
          <>
            <div class="recipe-top-bar">
              <div class="recipe-nav-links">
                <A href="/" class="back-link">
                  &larr; Back
                </A>
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
                  onClick={handleEnrich}
                  disabled={enriching() || isViewingHistoricalVersion()}
                >
                  {enriching() ? "Enriching..." : "Enrich with AI"}
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

            {/* Version History Panel */}
            <Show when={currentVersionId()}>
              <VersionHistoryPanel
                recipeId={params.id}
                currentVersionId={currentVersionId()!}
                onViewVersion={handleViewVersion}
                onRevertVersion={handleRevertClick}
                onCompareVersions={handleCompareVersions}
              />
            </Show>

            <div class="recipe-header-compact">
              <h2>{r().title}</h2>
              <Show when={r().tags && r().tags.length > 0}>
                <div class="recipe-tags">
                  <For each={r().tags}>
                    {(tag) => <span class="tag">{tag}</span>}
                  </For>
                </div>
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
                      <span class="value">{r().servings}</span>
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
                    <h3>Ingredients</h3>
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
                              {(ing) => (
                                <li>
                                  <Show when={ing.measurements[0]?.amount}>
                                    <span class="amount">
                                      {ing.measurements[0]?.amount}
                                    </span>{" "}
                                  </Show>
                                  <Show when={ing.measurements[0]?.unit}>
                                    <span class="unit">
                                      {ing.measurements[0]?.unit}
                                    </span>{" "}
                                  </Show>
                                  <Show when={ing.measurements.length > 1}>
                                    <span class="alt-measurement">
                                      (
                                      {ing.measurements
                                        .slice(1)
                                        .map((m) =>
                                          [m.amount, m.unit]
                                            .filter(Boolean)
                                            .join(" "),
                                        )
                                        .join(", ")}
                                      ){" "}
                                    </span>
                                  </Show>
                                  <span class="item">{ing.item}</span>
                                  <Show when={ing.note}>
                                    <span class="note"> ({ing.note})</span>
                                  </Show>
                                  <Show when={ing.raw}>
                                    <div class="ingredient-raw">
                                      Raw: {ing.raw}
                                    </div>
                                  </Show>
                                </li>
                              )}
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
            <Show when={enrichedContent() && recipe()}>
              <EnrichPreviewModal
                isOpen={() => enrichedContent() !== null}
                onClose={handleEnrichClose}
                currentRecipe={recipe()!}
                enrichedContent={enrichedContent()!}
                onApply={handleApplyEnrichment}
                applying={applyingEnrichment()}
              />
            </Show>

            {/* Version Compare Modal */}
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

            {/* Add to Shopping List Modal */}
            <AddToShoppingListModal
              isOpen={showShoppingListModal}
              onClose={() => setShowShoppingListModal(false)}
              recipe={r()}
            />

            {/* Add to Meal Plan Modal */}
            <Modal
              isOpen={showMealPlanModal}
              onClose={closeMealPlanModal}
              title="Add to Meal Plan"
              actions={
                <>
                  <button
                    type="button"
                    class="btn"
                    onClick={closeMealPlanModal}
                    disabled={addingToMealPlan()}
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    class="btn btn-primary"
                    onClick={handleAddToMealPlan}
                    disabled={addingToMealPlan() || mealPlanSuccess()}
                  >
                    {addingToMealPlan() ? "Adding..." : "Add"}
                  </button>
                </>
              }
            >
              <Show when={mealPlanSuccess()}>
                <div class="meal-plan-success">Added to meal plan!</div>
              </Show>
              <Show when={!mealPlanSuccess()}>
                <div class="meal-plan-form">
                  <div class="form-group">
                    <label for="meal-plan-date">Date</label>
                    <input
                      type="date"
                      id="meal-plan-date"
                      value={mealPlanDate()}
                      onInput={(e) => setMealPlanDate(e.currentTarget.value)}
                    />
                  </div>
                  <div class="form-group">
                    <label>Meal</label>
                    <div class="meal-type-buttons">
                      <For each={MEAL_TYPES}>
                        {(type) => (
                          <button
                            type="button"
                            class={`meal-type-button ${mealPlanMealType() === type ? "selected" : ""}`}
                            onClick={() => setMealPlanMealType(type)}
                          >
                            {MEAL_TYPE_LABELS[type]}
                          </button>
                        )}
                      </For>
                    </div>
                  </div>
                  <Show when={mealPlanError()}>
                    <p class="error">{mealPlanError()}</p>
                  </Show>
                </div>
              </Show>
            </Modal>
          </>
        )}
      </Show>
    </div>
  );
}
