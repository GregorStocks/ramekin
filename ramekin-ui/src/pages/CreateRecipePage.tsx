import { createSignal, createMemo, Show, onCleanup } from "solid-js";
import bookmarkletSource from "../bookmarklet.js?raw";
import { createStore } from "solid-js/store";
import { useNavigate, A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import RecipeForm from "../components/RecipeForm";
import { extractApiError } from "../utils/recipeFormHelpers";
import type { Ingredient, ScrapeJobResponse } from "ramekin-client";

export default function CreateRecipePage() {
  const navigate = useNavigate();
  const { getRecipesApi, getPhotosApi, getScrapeApi, refreshTags, token } =
    useAuth();

  // URL import state
  const [importUrl, setImportUrl] = createSignal("");
  const [scrapeJob, setScrapeJob] = createSignal<ScrapeJobResponse | null>(
    null,
  );
  const [scrapeError, setScrapeError] = createSignal<string | null>(null);
  const [scraping, setScraping] = createSignal(false);
  let pollInterval: ReturnType<typeof setInterval> | null = null;

  onCleanup(() => {
    if (pollInterval) clearInterval(pollInterval);
  });

  // Form state
  const [title, setTitle] = createSignal("");
  const [description, setDescription] = createSignal("");
  const [instructions, setInstructions] = createSignal("");
  const [sourceUrl, setSourceUrl] = createSignal("");
  const [sourceName, setSourceName] = createSignal("");
  const [tags, setTags] = createSignal<string[]>([]);
  const [photoIds, setPhotoIds] = createSignal<string[]>([]);
  const [uploading, setUploading] = createSignal(false);
  const [ingredients, setIngredients] = createStore<Ingredient[]>([
    { item: "", amount: "", unit: "" },
  ]);
  const [servings, setServings] = createSignal("");
  const [prepTime, setPrepTime] = createSignal("");
  const [cookTime, setCookTime] = createSignal("");
  const [totalTime, setTotalTime] = createSignal("");
  const [rating, setRating] = createSignal<number | null>(null);
  const [difficulty, setDifficulty] = createSignal("");
  const [nutritionalInfo, setNutritionalInfo] = createSignal("");
  const [notes, setNotes] = createSignal("");

  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [showBookmarklet, setShowBookmarklet] = createSignal(false);

  const bookmarkletCode = createMemo(() => {
    const origin = window.location.origin;
    // Use UI origin for API calls - Vite proxy forwards /api/* to the API server
    const apiOrigin = origin;
    const userToken = token();
    if (!userToken) return "";
    const code = bookmarkletSource
      .replace("__ORIGIN__", origin)
      .replace("__TOKEN__", userToken)
      .replace("__API__", encodeURIComponent(apiOrigin));
    // Minify: remove newlines, collapse whitespace
    const minified = code
      .replace(/\n\s*/g, "")
      .replace(/\s+/g, " ")
      .replace(/\s*([{}();,:])\s*/g, "$1")
      .trim();
    return `javascript:${minified}`;
  });

  const startScrape = async () => {
    const url = importUrl().trim();
    if (!url) return;

    setScrapeError(null);
    setScrapeJob(null);
    setScraping(true);

    try {
      const response = await getScrapeApi().createScrape({
        createScrapeRequest: { url },
      });

      setScrapeJob({ ...response, url, canRetry: false, retryCount: 0 });

      // Start polling
      pollInterval = setInterval(async () => {
        try {
          const job = await getScrapeApi().getScrape({ id: response.id });
          setScrapeJob(job);

          if (job.status === "completed" || job.status === "failed") {
            if (pollInterval) clearInterval(pollInterval);
            pollInterval = null;
            setScraping(false);
            if (job.status === "failed") {
              setScrapeError(job.error || "Import failed");
            }
          }
        } catch {
          if (pollInterval) clearInterval(pollInterval);
          pollInterval = null;
          setScraping(false);
          setScrapeError("Failed to check import status");
        }
      }, 1000);
    } catch (err) {
      setScraping(false);
      const errorMessage = await extractApiError(err, "Failed to start import");
      setScrapeError(errorMessage);
    }
  };

  const retryScrape = async () => {
    const job = scrapeJob();
    if (!job) return;

    setScrapeError(null);
    setScraping(true);

    try {
      await getScrapeApi().retryScrape({ id: job.id });

      // Start polling again
      pollInterval = setInterval(async () => {
        try {
          const updatedJob = await getScrapeApi().getScrape({ id: job.id });
          setScrapeJob(updatedJob);

          if (
            updatedJob.status === "completed" ||
            updatedJob.status === "failed"
          ) {
            if (pollInterval) clearInterval(pollInterval);
            pollInterval = null;
            setScraping(false);
            if (updatedJob.status === "failed") {
              setScrapeError(updatedJob.error || "Import failed");
            }
          }
        } catch {
          if (pollInterval) clearInterval(pollInterval);
          pollInterval = null;
          setScraping(false);
          setScrapeError("Failed to check import status");
        }
      }, 1000);
    } catch {
      setScraping(false);
      setScrapeError("Failed to retry import");
    }
  };

  const clearImport = () => {
    setScrapeJob(null);
    setScrapeError(null);
    setImportUrl("");
  };

  const getScrapeStatusText = () => {
    const job = scrapeJob();
    if (!job) return "";
    switch (job.status) {
      case "pending":
        return "Starting...";
      case "scraping":
        return "Fetching page...";
      case "parsing":
        return "Extracting recipe...";
      case "completed":
        return "Done!";
      case "failed":
        return "Failed";
      default:
        return job.status;
    }
  };

  const handlePhotoUpload = async (e: Event) => {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    setUploading(true);
    setError(null);
    try {
      const response = await getPhotosApi().upload({ file });
      setPhotoIds([...photoIds(), response.id]);
    } catch (err) {
      const errorMessage = await extractApiError(err, "Failed to upload photo");
      setError(errorMessage);
    } finally {
      setUploading(false);
      input.value = "";
    }
  };

  const removePhoto = (photoId: string) => {
    setPhotoIds(photoIds().filter((id) => id !== photoId));
  };

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    setError(null);
    setSaving(true);

    try {
      const validIngredients = ingredients.filter(
        (ing) => ing.item.trim() !== "",
      );

      const response = await getRecipesApi().createRecipe({
        createRecipeRequest: {
          title: title(),
          description: description() || undefined,
          instructions: instructions(),
          ingredients: validIngredients,
          sourceUrl: sourceUrl() || undefined,
          sourceName: sourceName() || undefined,
          tags: tags().length > 0 ? tags() : undefined,
          photoIds: photoIds().length > 0 ? photoIds() : undefined,
          servings: servings() || undefined,
          prepTime: prepTime() || undefined,
          cookTime: cookTime() || undefined,
          totalTime: totalTime() || undefined,
          rating: rating() ?? undefined,
          difficulty: difficulty() || undefined,
          nutritionalInfo: nutritionalInfo() || undefined,
          notes: notes() || undefined,
        },
      });

      // Refresh tags cache in case new tags were created
      refreshTags();

      navigate(`/recipes/${response.id}`);
    } catch (err) {
      const errorMessage = await extractApiError(
        err,
        "Failed to create recipe",
      );
      setError(errorMessage);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div class="create-recipe-page">
      <h2>Create New Recipe</h2>

      {/* URL Import Section */}
      <div class="import-section">
        <div class="import-header">
          <label>Import from URL</label>
        </div>
        <Show
          when={scrapeJob()?.status !== "completed"}
          fallback={
            <div class="import-success">
              <span>Recipe imported!</span>
              <A
                href={`/recipes/${scrapeJob()?.recipeId}`}
                class="btn btn-small"
              >
                View
              </A>
              <A
                href={`/recipes/${scrapeJob()?.recipeId}/edit`}
                class="btn btn-small"
              >
                Edit
              </A>
              <button type="button" class="btn btn-small" onClick={clearImport}>
                Import another
              </button>
            </div>
          }
        >
          <div class="import-row">
            <input
              type="url"
              placeholder="Paste recipe URL..."
              value={importUrl()}
              onInput={(e) => setImportUrl(e.currentTarget.value)}
              disabled={scraping()}
              class="import-input"
            />
            <button
              type="button"
              class="btn btn-primary"
              onClick={startScrape}
              disabled={scraping() || !importUrl().trim()}
            >
              {scraping() ? getScrapeStatusText() : "Import"}
            </button>
          </div>
          <Show when={scrapeError()}>
            <div class="import-error">
              <span>{scrapeError()}</span>
              <Show when={scrapeJob()?.canRetry}>
                <button
                  type="button"
                  class="btn btn-small"
                  onClick={retryScrape}
                  disabled={scraping()}
                >
                  Retry
                </button>
              </Show>
            </div>
          </Show>
          <p class="import-hint">
            Import a recipe from a website. Works with sites that use structured
            recipe data.
          </p>
        </Show>
      </div>

      {/* Bookmarklet Section */}
      <div class="bookmarklet-section">
        <button
          type="button"
          class="bookmarklet-toggle"
          onClick={() => setShowBookmarklet(!showBookmarklet())}
        >
          {showBookmarklet() ? "Hide" : "Show"} Bookmarklet
        </button>
        <Show when={showBookmarklet()}>
          <div class="bookmarklet-content">
            <p>
              Drag this link to your bookmarks bar to capture recipes from any
              page:
            </p>
            <a href={bookmarkletCode()} class="bookmarklet-link">
              Save to Ramekin
            </a>
            <p class="bookmarklet-hint">
              This works even on paywalled sites when you're logged in.
            </p>
          </div>
        </Show>
      </div>

      <div class="section-divider">
        <span>or enter manually</span>
      </div>

      <RecipeForm
        title={title}
        setTitle={setTitle}
        description={description}
        setDescription={setDescription}
        instructions={instructions}
        setInstructions={setInstructions}
        sourceUrl={sourceUrl}
        setSourceUrl={setSourceUrl}
        sourceName={sourceName}
        setSourceName={setSourceName}
        tags={tags}
        setTags={setTags}
        servings={servings}
        setServings={setServings}
        prepTime={prepTime}
        setPrepTime={setPrepTime}
        cookTime={cookTime}
        setCookTime={setCookTime}
        totalTime={totalTime}
        setTotalTime={setTotalTime}
        rating={rating}
        setRating={setRating}
        difficulty={difficulty}
        setDifficulty={setDifficulty}
        nutritionalInfo={nutritionalInfo}
        setNutritionalInfo={setNutritionalInfo}
        notes={notes}
        setNotes={setNotes}
        ingredients={ingredients}
        setIngredients={setIngredients}
        photoIds={photoIds}
        onPhotoUpload={handlePhotoUpload}
        onPhotoRemove={removePhoto}
        uploading={uploading}
        saving={saving}
        error={error}
        onSubmit={handleSubmit}
        submitLabel="Create Recipe"
        submitLabelSaving="Creating..."
        cancelHref="/"
        token={token}
      />
    </div>
  );
}
