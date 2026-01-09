import { createSignal, Show, For, onMount, onCleanup } from "solid-js";
import { useParams, A, useNavigate } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import StarRating from "../components/StarRating";
import type { RecipeResponse } from "ramekin-client";

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

export default function ViewRecipePage() {
  const params = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { getRecipesApi, token } = useAuth();

  const [recipe, setRecipe] = createSignal<RecipeResponse | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [deleting, setDeleting] = createSignal(false);
  const [checkedIngredients, setCheckedIngredients] = createSignal<Set<number>>(
    new Set(),
  );

  const toggleIngredient = (index: number) => {
    setCheckedIngredients((prev) => {
      const next = new Set(prev);
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      return next;
    });
  };

  const loadRecipe = async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await getRecipesApi().getRecipe({ id: params.id });
      setRecipe(response);
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

  onMount(() => {
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
              <A href="/" class="back-link">
                &larr; Back
              </A>
              <div class="recipe-actions">
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
                    <ul class="ingredients-list">
                      <For each={r().ingredients}>
                        {(ing, index) => (
                          <li onClick={() => toggleIngredient(index())}>
                            <div
                              class={`ingredient-checkbox ${checkedIngredients().has(index()) ? "checked" : ""}`}
                            />
                            <span
                              class={`ingredient-text ${checkedIngredients().has(index()) ? "checked" : ""}`}
                            >
                              <Show when={ing.amount}>
                                <span class="amount">{ing.amount}</span>{" "}
                              </Show>
                              <Show when={ing.unit}>
                                <span class="unit">{ing.unit}</span>{" "}
                              </Show>
                              <span class="item">{ing.item}</span>
                              <Show when={ing.note}>
                                <span class="note"> ({ing.note})</span>
                              </Show>
                            </span>
                          </li>
                        )}
                      </For>
                    </ul>
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
          </>
        )}
      </Show>
    </div>
  );
}
