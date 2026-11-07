import { createSignal, createMemo, Show } from "solid-js";
import bookmarkletSource from "../bookmarklet.js?raw";
import { useNavigate } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import { usePageTitle } from "../utils/pageTitle";
import { useUnsavedChangesGuard } from "../utils/unsavedChangesGuard";
import { createStartFlow } from "../utils/startFlow";

declare const __EXTERNAL_URL__: string;

export default function CreateRecipePage() {
  usePageTitle(() => "New Recipe");
  const navigate = useNavigate();
  const { getImportApi, getScrapeApi, getUsersApi } = useAuth();
  const [recipeText, setRecipeText] = createSignal("");
  const textImport = createStartFlow(
    () =>
      getImportApi().importFromText({
        importFromTextRequest: { text: recipeText() },
      }),
    (response) => navigate(`/scrape/${response.jobId}`),
    "Could not start the import. Your text is unchanged; try again.",
  );

  // Pasted text is work worth asking about before leaving.
  useUnsavedChangesGuard(
    () => textImport.state().kind !== "started" && recipeText().trim() !== "",
  );

  // URL import state
  const [importUrl, setImportUrl] = createSignal("");
  const scrape = createStartFlow(
    () =>
      getScrapeApi().createScrape({
        createScrapeRequest: { url: importUrl().trim() },
      }),
    (response) => navigate(`/scrape/${response.id}`),
    "Failed to start import",
  );

  const [showBookmarklet, setShowBookmarklet] = createSignal(false);
  // A long-lived, capture-scoped token minted just for the bookmarklet. Unlike
  // the login session token it doesn't expire, so a saved bookmarklet keeps
  // working. Minting a fresh one never invalidates previously-saved ones.
  const [bookmarkletToken, setBookmarkletToken] = createSignal<string | null>(
    null,
  );
  const mintBookmarklet = createStartFlow(
    () => getUsersApi().mintBookmarkletToken(),
    (response) => setBookmarkletToken(response.token),
    "Failed to create bookmarklet",
  );

  const toggleBookmarklet = () => {
    const next = !showBookmarklet();
    setShowBookmarklet(next);
    // Mint the token lazily, the first time the section is opened. The flow
    // ignores a toggle while a mint is in flight or already done.
    if (next) void mintBookmarklet.run();
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

  const startScrape = () => {
    if (!importUrl().trim()) return;
    void scrape.run();
  };

  return (
    <div class="create-recipe-page">
      <h2>Create New Recipe</h2>

      <div class="form-group">
        <label for="recipe-text">Recipe text</label>
        <textarea
          id="recipe-text"
          rows={14}
          value={recipeText()}
          onInput={(e) => setRecipeText(e.currentTarget.value)}
          disabled={textImport.busy()}
          placeholder="Type or paste a whole recipe: title, ingredients, instructions, and any details."
        />
      </div>
      <button
        class="btn btn-primary"
        onClick={() => void textImport.run()}
        disabled={textImport.busy() || !recipeText().trim()}
      >
        {textImport.busy() ? "Starting…" : "Import recipe"}
      </button>
      <Show when={textImport.error()}>
        <p role="alert">{textImport.error()}</p>
      </Show>

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
            disabled={scrape.busy()}
            class="import-input"
          />
          <button
            type="button"
            class="btn btn-primary"
            onClick={startScrape}
            disabled={scrape.busy() || !importUrl().trim()}
          >
            {scrape.busy() ? "Starting..." : "Import"}
          </button>
        </div>
        <Show when={scrape.error()}>
          <div class="import-error">
            <span>{scrape.error()}</span>
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
                  {mintBookmarklet.error() ?? "Generating bookmarklet…"}
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
    </div>
  );
}
