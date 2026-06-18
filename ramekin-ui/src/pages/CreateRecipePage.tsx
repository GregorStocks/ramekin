import { createSignal, createMemo, Show, onCleanup, onMount } from "solid-js";
import bookmarkletSource from "../bookmarklet.js?raw";
import { createStore } from "solid-js/store";
import { useNavigate } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import RecipeForm from "../components/RecipeForm";
import { extractApiError, extractImageFile } from "../utils/recipeFormHelpers";
import { usePageTitle } from "../utils/pageTitle";
import type { Ingredient } from "ramekin-client";

declare const __EXTERNAL_URL__: string;

export default function CreateRecipePage() {
  usePageTitle(() => "New Recipe");
  const navigate = useNavigate();
  const {
    getRecipesApi,
    getPhotosApi,
    getScrapeApi,
    getUsersApi,
    refreshTags,
    token,
  } = useAuth();

  // URL import state
  const [importUrl, setImportUrl] = createSignal("");
  const [scrapeError, setScrapeError] = createSignal<string | null>(null);
  const [scraping, setScraping] = createSignal(false);

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
    { item: "", measurements: [{}] },
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
  // A long-lived, capture-scoped token minted just for the bookmarklet. Unlike
  // the login session token it doesn't expire, so a saved bookmarklet keeps
  // working. Minting a fresh one never invalidates previously-saved ones.
  const [bookmarkletToken, setBookmarkletToken] = createSignal<string | null>(
    null,
  );
  const [bookmarkletError, setBookmarkletError] = createSignal<string | null>(
    null,
  );

  const toggleBookmarklet = async () => {
    const next = !showBookmarklet();
    setShowBookmarklet(next);
    // Mint the token lazily, the first time the section is opened.
    if (next && !bookmarkletToken()) {
      setBookmarkletError(null);
      try {
        const response = await getUsersApi().mintBookmarkletToken();
        setBookmarkletToken(response.token);
      } catch (err) {
        setBookmarkletError(
          await extractApiError(err, "Failed to create bookmarklet"),
        );
      }
    }
  };

  const bookmarkletCode = createMemo(() => {
    const origin = window.location.origin;
    // Use UI origin for API calls - Vite proxy forwards /api/* to the API server
    const apiOrigin = origin;
    const bmToken = bookmarkletToken();
    if (!bmToken) return "";
    const code = bookmarkletSource
      .replace("__ORIGIN__", origin)
      .replace("__TOKEN__", bmToken)
      .replace("__API__", encodeURIComponent(apiOrigin))
      .replace(
        "__EXTERNAL__",
        encodeURIComponent(__EXTERNAL_URL__.replace(/\/+$/, "")),
      );
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
    setScraping(true);

    try {
      const response = await getScrapeApi().createScrape({
        createScrapeRequest: { url },
      });
      navigate(`/scrape/${response.id}`);
    } catch (err) {
      setScraping(false);
      const errorMessage = await extractApiError(err, "Failed to start import");
      setScrapeError(errorMessage);
    }
  };

  const uploadPhotoFile = async (file: File) => {
    if (uploading()) return;
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
    }
  };

  const handlePhotoUpload = async (e: Event) => {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    try {
      await uploadPhotoFile(file);
    } finally {
      input.value = "";
    }
  };

  const handlePaste = (e: ClipboardEvent) => {
    const file = extractImageFile(e.clipboardData);
    if (!file) return;
    e.preventDefault();
    void uploadPhotoFile(file);
  };

  onMount(() => {
    document.addEventListener("paste", handlePaste);
  });

  onCleanup(() => {
    document.removeEventListener("paste", handlePaste);
  });

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
            {scraping() ? "Starting..." : "Import"}
          </button>
        </div>
        <Show when={scrapeError()}>
          <div class="import-error">
            <span>{scrapeError()}</span>
          </div>
        </Show>
        <p class="import-hint">
          Import a recipe from a website. Works with sites that use structured
          recipe data.
        </p>
      </div>

      {/* Bookmarklet Section */}
      <div class="bookmarklet-section">
        <button
          type="button"
          class="bookmarklet-toggle"
          onClick={toggleBookmarklet}
        >
          {showBookmarklet() ? "Hide" : "Show"} Bookmarklet
        </button>
        <Show when={showBookmarklet()}>
          <div class="bookmarklet-content">
            <p>
              Drag this link to your bookmarks bar to capture recipes from any
              page:
            </p>
            <Show
              when={bookmarkletToken()}
              fallback={
                <p class="bookmarklet-hint">
                  {bookmarkletError() ?? "Generating bookmarklet…"}
                </p>
              }
            >
              <a href={bookmarkletCode()} class="bookmarklet-link">
                Save to Ramekin
              </a>
              <p class="bookmarklet-hint">
                This works even on paywalled sites when you're logged in. The
                link keeps working indefinitely — no need to regenerate it.
              </p>
            </Show>
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
