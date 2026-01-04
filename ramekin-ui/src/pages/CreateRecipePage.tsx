import { createSignal, Show, Index, For, onCleanup } from "solid-js";
import { createStore } from "solid-js/store";
import { useNavigate, A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import TagInput from "../components/TagInput";
import type { Ingredient } from "ramekin-client";

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
  const { getRecipesApi, getPhotosApi, token } = useAuth();

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
