import { createSignal, Show, Index, For, onMount, onCleanup } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { useParams, useNavigate, A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import TagInput from "../components/TagInput";
import StarRating from "../components/StarRating";
import type { Ingredient, RecipeResponse } from "ramekin-client";

function PhotoThumbnail(props: {
  photoId: string;
  token: string;
  onRemove: () => void;
}) {
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

export default function EditRecipePage() {
  const params = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { getRecipesApi, getPhotosApi, token } = useAuth();

  const [loading, setLoading] = createSignal(true);
  const [title, setTitle] = createSignal("");
  const [description, setDescription] = createSignal("");
  const [photoIds, setPhotoIds] = createSignal<string[]>([]);
  const [uploading, setUploading] = createSignal(false);
  const [instructions, setInstructions] = createSignal("");
  const [sourceUrl, setSourceUrl] = createSignal("");
  const [sourceName, setSourceName] = createSignal("");
  const [tags, setTags] = createSignal<string[]>([]);
  const [ingredients, setIngredients] = createStore<Ingredient[]>([]);
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

  const loadRecipe = async () => {
    setLoading(true);
    setError(null);
    try {
      const response: RecipeResponse = await getRecipesApi().getRecipe({
        id: params.id,
      });
      setTitle(response.title);
      setDescription(response.description || "");
      setInstructions(response.instructions);
      setSourceUrl(response.sourceUrl || "");
      setSourceName(response.sourceName || "");
      setTags(response.tags || []);
      setPhotoIds(response.photoIds || []);
      setIngredients(
        reconcile(
          response.ingredients?.length
            ? response.ingredients
            : [{ item: "", amount: "", unit: "" }],
        ),
      );
      setServings(response.servings || "");
      setPrepTime(response.prepTime || "");
      setCookTime(response.cookTime || "");
      setTotalTime(response.totalTime || "");
      setRating(response.rating ?? null);
      setDifficulty(response.difficulty || "");
      setNutritionalInfo(response.nutritionalInfo || "");
      setNotes(response.notes || "");
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

  onMount(() => {
    loadRecipe();
  });

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

      await getRecipesApi().updateRecipe({
        id: params.id,
        updateRecipeRequest: {
          title: title(),
          description: description() || undefined,
          instructions: instructions(),
          ingredients: validIngredients,
          sourceUrl: sourceUrl() || undefined,
          sourceName: sourceName() || undefined,
          tags: tags().length > 0 ? tags() : undefined,
          photoIds: photoIds(),
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

      navigate(`/recipes/${params.id}`);
    } catch (err) {
      if (err instanceof Response) {
        const body = await err.json();
        setError(body.error || "Failed to update recipe");
      } else {
        setError("Failed to update recipe");
      }
    } finally {
      setSaving(false);
    }
  };

  return (
    <div class="edit-recipe-page">
      <h2>Edit Recipe</h2>

      <Show when={loading()}>
        <p class="loading">Loading recipe...</p>
      </Show>

      <Show when={error() && loading()}>
        <div class="error-state">
          <p class="error">{error()}</p>
          <A href="/" class="btn">
            Back to Cookbook
          </A>
        </div>
      </Show>

      <Show when={!loading()}>
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

          <div class="form-row-4">
            <div class="form-group">
              <label for="servings">Servings</label>
              <input
                id="servings"
                type="text"
                value={servings()}
                onInput={(e) => setServings(e.currentTarget.value)}
                placeholder="e.g., 4"
              />
            </div>
            <div class="form-group">
              <label for="prepTime">Prep Time</label>
              <input
                id="prepTime"
                type="text"
                value={prepTime()}
                onInput={(e) => setPrepTime(e.currentTarget.value)}
                placeholder="e.g., 15 min"
              />
            </div>
            <div class="form-group">
              <label for="cookTime">Cook Time</label>
              <input
                id="cookTime"
                type="text"
                value={cookTime()}
                onInput={(e) => setCookTime(e.currentTarget.value)}
                placeholder="e.g., 30 min"
              />
            </div>
            <div class="form-group">
              <label for="totalTime">Total Time</label>
              <input
                id="totalTime"
                type="text"
                value={totalTime()}
                onInput={(e) => setTotalTime(e.currentTarget.value)}
                placeholder="e.g., 45 min"
              />
            </div>
          </div>

          <div class="form-row">
            <div class="form-group-rating">
              <label>Rating</label>
              <div class="rating-input-wrapper">
                <StarRating rating={rating()} onRate={setRating} />
                <Show when={rating() !== null}>
                  <button
                    type="button"
                    class="rating-clear"
                    onClick={() => setRating(null)}
                  >
                    Clear
                  </button>
                </Show>
              </div>
            </div>
            <div class="form-group">
              <label for="difficulty">Difficulty</label>
              <input
                id="difficulty"
                type="text"
                value={difficulty()}
                onInput={(e) => setDifficulty(e.currentTarget.value)}
                placeholder="e.g., Easy, Medium, Hard"
              />
            </div>
          </div>

          <div class="form-section">
            <div class="section-header">
              <label>Ingredients</label>
              <button
                type="button"
                class="btn btn-small"
                onClick={addIngredient}
              >
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
                    class="btn btn-small btn-remove"
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

          <div class="form-group">
            <label for="notes">Notes</label>
            <textarea
              id="notes"
              value={notes()}
              onInput={(e) => setNotes(e.currentTarget.value)}
              rows={3}
              placeholder="Additional notes, tips, or variations..."
            />
          </div>

          <div class="form-group">
            <label for="nutritionalInfo">Nutritional Info</label>
            <textarea
              id="nutritionalInfo"
              value={nutritionalInfo()}
              onInput={(e) => setNutritionalInfo(e.currentTarget.value)}
              rows={2}
              placeholder="Calories, protein, carbs, etc."
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
            <A href={`/recipes/${params.id}`} class="btn">
              Cancel
            </A>
            <button type="submit" class="btn btn-primary" disabled={saving()}>
              {saving() ? "Saving..." : "Save Changes"}
            </button>
          </div>
        </form>
      </Show>
    </div>
  );
}
