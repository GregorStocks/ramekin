import { createSignal, createMemo, Show } from "solid-js";
import bookmarkletSource from "../bookmarklet.js?raw";
import { useNavigate } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import RecipeForm from "../components/RecipeForm";
import { extractApiError } from "../utils/recipeFormHelpers";
import { createRecipeFormState } from "../utils/recipeFormState";
import { usePageTitle } from "../utils/pageTitle";

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
  const form = createRecipeFormState({ getPhotosApi });

  // URL import state
  const [importUrl, setImportUrl] = createSignal("");
  const [scrapeError, setScrapeError] = createSignal<string | null>(null);
  const [scraping, setScraping] = createSignal(false);

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

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    form.setError(null);
    form.setSaving(true);

    try {
      const response = await getRecipesApi().createRecipe({
        createRecipeRequest: form.toCreateRecipeRequest(),
      });

      // Refresh tags cache in case new tags were created
      refreshTags();

      navigate(`/recipes/${response.id}`);
    } catch (err) {
      const errorMessage = await extractApiError(
        err,
        "Failed to create recipe",
      );
      form.setError(errorMessage);
    } finally {
      form.setSaving(false);
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
        form={form}
        onSubmit={handleSubmit}
        submitLabel="Create Recipe"
        submitLabelSaving="Creating..."
        cancelHref="/"
        token={token}
      />
    </div>
  );
}
