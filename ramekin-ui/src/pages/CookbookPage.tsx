import { createSignal, Show, For, onMount } from "solid-js";
import { A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import type { RecipeSummary } from "ramekin-client";

function formatRelativeDate(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) {
    return "Updated today";
  } else if (diffDays === 1) {
    return "Updated yesterday";
  } else if (diffDays < 7) {
    return `Updated ${diffDays} days ago`;
  } else {
    return `Updated ${date.toLocaleDateString("en-US", { month: "short", day: "numeric" })}`;
  }
}

export default function CookbookPage() {
  const { getRecipesApi } = useAuth();

  const [recipes, setRecipes] = createSignal<RecipeSummary[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  const loadRecipes = async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await getRecipesApi().listRecipes();
      setRecipes(response.recipes);
    } catch (err) {
      setError("Failed to load recipes");
    } finally {
      setLoading(false);
    }
  };

  onMount(() => {
    loadRecipes();
  });

  const recipeCount = () => {
    const count = recipes().length;
    if (count === 0) return "";
    if (count === 1) return "(1 recipe)";
    return `(${count} recipes)`;
  };

  return (
    <div class="cookbook-page">
      <div class="page-header">
        <h2>
          My Cookbook{" "}
          <Show when={!loading() && recipes().length > 0}>
            <span class="recipe-count">{recipeCount()}</span>
          </Show>
        </h2>
        <A href="/recipes/new" class="btn btn-primary">
          + New Recipe
        </A>
      </div>

      <Show when={loading()}>
        <p class="loading">Loading recipes...</p>
      </Show>

      <Show when={error()}>
        <p class="error">{error()}</p>
      </Show>

      <Show when={!loading() && recipes().length === 0}>
        <div class="empty-state">
          <div class="empty-state-icon">📖</div>
          <h3>Your cookbook is empty</h3>
          <p>Start building your collection by adding your first recipe.</p>
          <A href="/recipes/new" class="btn btn-primary">
            + Add Your First Recipe
          </A>
        </div>
      </Show>

      <Show when={!loading() && recipes().length > 0}>
        <div class="recipe-grid">
          <For each={recipes()}>
            {(recipe) => (
              <A href={`/recipes/${recipe.id}`} class="recipe-card">
                <Show
                  when={recipe.thumbnail}
                  fallback={<div class="recipe-card-placeholder">🍽️</div>}
                >
                  <img
                    src={`data:image/jpeg;base64,${recipe.thumbnail}`}
                    alt=""
                    class="recipe-card-thumbnail"
                  />
                </Show>
                <div class="recipe-card-content">
                  <h3>{recipe.title}</h3>
                  <Show when={recipe.description}>
                    <p class="recipe-description">{recipe.description}</p>
                  </Show>
                  <Show when={recipe.tags && recipe.tags.length > 0}>
                    <div class="recipe-tags">
                      <For each={recipe.tags!.slice(0, 3)}>
                        {(tag) => <span class="tag">{tag}</span>}
                      </For>
                      <Show when={recipe.tags!.length > 3}>
                        <span class="tag tag-more">
                          +{recipe.tags!.length - 3}
                        </span>
                      </Show>
                    </div>
                  </Show>
                  <p class="recipe-date">
                    {formatRelativeDate(recipe.updatedAt)}
                  </p>
                </div>
              </A>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
