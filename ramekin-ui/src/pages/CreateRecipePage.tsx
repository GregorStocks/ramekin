import {
  createSignal,
  createMemo,
  Show,
  Index,
  For,
  onCleanup,
} from "solid-js";
import bookmarkletSource from "../bookmarklet.js?raw";
import { createStore } from "solid-js/store";
import { useNavigate, A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import TagInput from "../components/TagInput";
import type { Ingredient, ScrapeJobResponse } from "ramekin-client";

function PhotoThumbnail(props: {
  photoId: string;
  token: string;
  onRemove: () => void;
}) {
  const [src, setSrc] = createSignal<string | null>(null);

  (async () => {
    const response = await fetch(`/api/photos/${props.photoId}`, {
      headers: { Authorization: `Bearer ${props.token}` },
    });
    if (response.ok) {
      const blob = await response.blob();
      setSrc(URL.createObjectURL(blob));
    }
  })();

  onCleanup(() => {
    const url = src();
    if (url) URL.revokeObjectURL(url);
  });

  return (
    <div class="photo-thumbnail">
      <Show when={src()} fallback={<div class="photo-loading">Loading...</div>}>
        <img src={src()!} alt="Recipe photo" />
      </Show>
      <button type="button" class="photo-remove" onClick={props.onRemove}>
        &times;
      </button>
    </div>
  );
}

export default function CreateRecipePage() {
  const navigate = useNavigate();
  const { getRecipesApi, getPhotosApi, getScrapeApi, token } = useAuth();

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

  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [showBookmarklet, setShowBookmarklet] = createSignal(false);

  const bookmarkletCode = createMemo(() => {
    const origin = window.location.origin;
    // API server is on port 3000 (HTTP for now)
    const apiOrigin = `http://${window.location.hostname}:3000`;
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
        } catch (err) {
          if (pollInterval) clearInterval(pollInterval);
          pollInterval = null;
          setScraping(false);
          setScrapeError("Failed to check import status");
        }
      }, 1000);
    } catch (err) {
      setScraping(false);
      // Handle API errors
      const response =
        err instanceof Response
          ? err
          : err &&
              typeof err === "object" &&
              "response" in err &&
              err.response instanceof Response
            ? err.response
            : null;

      if (response) {
        try {
          const body = await response.json();
          setScrapeError(body.error || "Failed to start import");
        } catch {
          setScrapeError(`Failed to start import (${response.status})`);
        }
      } else {
        setScrapeError("Failed to start import");
      }
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
        } catch (err) {
          if (pollInterval) clearInterval(pollInterval);
          pollInterval = null;
          setScraping(false);
          setScrapeError("Failed to check import status");
        }
      }, 1000);
    } catch (err) {
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

  const addIngredient = () => {
    setIngredients(ingredients.length, { item: "", amount: "", unit: "" });
  };

  const removeIngredient = (index: number) => {
    setIngredients((ings) => ings.filter((_, i) => i !== index));
  };

  const updateIngredient = (
    index: number,
    field: keyof Ingredient,
    value: string,
  ) => {
    setIngredients(index, field, value);
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
      // The generated client throws ResponseError with a response property
      const response =
        err instanceof Response
          ? err
          : err &&
              typeof err === "object" &&
              "response" in err &&
              err.response instanceof Response
            ? err.response
            : null;

      if (response) {
        try {
          const body = await response.json();
          setError(body.error || "Failed to upload photo");
        } catch {
          setError(`Failed to upload photo (${response.status})`);
        }
      } else {
        setError("Failed to upload photo");
      }
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
        },
      });

      navigate(`/recipes/${response.id}`);
    } catch (err) {
      if (err instanceof Response) {
        const body = await err.json();
        setError(body.error || "Failed to create recipe");
      } else {
        setError("Failed to create recipe");
      }
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

      <form onSubmit={handleSubmit}>
        <div class="form-group">
          <label for="title">Title *</label>
          <input
            id="title"
            type="text"
            value={title()}
            onInput={(e) => setTitle(e.currentTarget.value)}
            required
          />
        </div>

        <div class="form-group">
          <label for="description">Description</label>
          <textarea
            id="description"
            value={description()}
            onInput={(e) => setDescription(e.currentTarget.value)}
            rows={2}
          />
        </div>

        <div class="form-section">
          <div class="section-header">
            <label>Ingredients</label>
            <button type="button" class="btn btn-small" onClick={addIngredient}>
              + Add
            </button>
          </div>
          <Index each={ingredients}>
            {(ing, index) => (
              <div class="ingredient-row">
                <input
                  type="text"
                  placeholder="Amount"
                  value={ing().amount || ""}
                  onInput={(e) =>
                    updateIngredient(index, "amount", e.currentTarget.value)
                  }
                  class="input-amount"
                />
                <input
                  type="text"
                  placeholder="Unit"
                  value={ing().unit || ""}
                  onInput={(e) =>
                    updateIngredient(index, "unit", e.currentTarget.value)
                  }
                  class="input-unit"
                />
                <input
                  type="text"
                  placeholder="Ingredient *"
                  value={ing().item}
                  onInput={(e) =>
                    updateIngredient(index, "item", e.currentTarget.value)
                  }
                  class="input-item"
                />
                <input
                  type="text"
                  placeholder="Note"
                  value={ing().note || ""}
                  onInput={(e) =>
                    updateIngredient(index, "note", e.currentTarget.value)
                  }
                  class="input-note"
                />
                <button
                  type="button"
                  class="btn btn-small btn-danger"
                  onClick={() => removeIngredient(index)}
                >
                  &times;
                </button>
              </div>
            )}
          </Index>
        </div>

        <div class="form-group">
          <label for="instructions">Instructions *</label>
          <textarea
            id="instructions"
            value={instructions()}
            onInput={(e) => setInstructions(e.currentTarget.value)}
            rows={8}
            required
          />
        </div>

        <div class="form-row">
          <div class="form-group">
            <label for="sourceUrl">Source URL</label>
            <input
              id="sourceUrl"
              type="url"
              value={sourceUrl()}
              onInput={(e) => setSourceUrl(e.currentTarget.value)}
              placeholder="https://..."
            />
          </div>
          <div class="form-group">
            <label for="sourceName">Source Name</label>
            <input
              id="sourceName"
              type="text"
              value={sourceName()}
              onInput={(e) => setSourceName(e.currentTarget.value)}
              placeholder="e.g., Grandma's cookbook"
            />
          </div>
        </div>

        <div class="form-group">
          <label for="tags">Tags</label>
          <TagInput
            id="tags"
            tags={tags}
            onTagsChange={setTags}
            placeholder="e.g., dinner, easy, vegetarian"
          />
        </div>

        <div class="form-section">
          <div class="section-header">
            <label>Photos</label>
            <label class="btn btn-small">
              {uploading() ? "Uploading..." : "+ Add Photo"}
              <input
                type="file"
                accept="image/*"
                onChange={handlePhotoUpload}
                disabled={uploading()}
                style={{ display: "none" }}
              />
            </label>
          </div>
          <Show when={photoIds().length > 0}>
            <div class="photo-grid">
              <For each={photoIds()}>
                {(photoId) => (
                  <PhotoThumbnail
                    photoId={photoId}
                    token={token() ?? ""}
                    onRemove={() => removePhoto(photoId)}
                  />
                )}
              </For>
            </div>
          </Show>
          <Show when={photoIds().length === 0}>
            <p class="empty-photos">No photos yet</p>
          </Show>
        </div>

        <Show when={error()}>
          <div class="error">{error()}</div>
        </Show>

        <div class="form-actions">
          <A href="/" class="btn">
            Cancel
          </A>
          <button type="submit" class="btn btn-primary" disabled={saving()}>
            {saving() ? "Creating..." : "Create Recipe"}
          </button>
        </div>
      </form>
    </div>
  );
}
