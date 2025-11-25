import { createSignal, Show, For, onMount } from "solid-js";
import { A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import type { RecipeSummary } from "ramekin-client";

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

  return (
    <div class="cookbook-page">
      <div class="page-header">
        <h2>My Cookbook</h2>
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
          <p>No recipes yet. Create your first recipe!</p>
          <A href="/recipes/new" class="btn btn-primary">
            Create Recipe
          </A>
        </div>
      </Show>

      <div class="recipe-grid">
        <For each={recipes()}>
          {(recipe) => (
            <A href={`/recipes/${recipe.id}`} class="recipe-card">
              <h3>{recipe.title}</h3>
              <Show when={recipe.description}>
                <p class="recipe-description">{recipe.description}</p>
              </Show>
              <Show when={recipe.tags && recipe.tags.length > 0}>
                <div class="recipe-tags">
                  <For each={recipe.tags}>
                    {(tag) => <span class="tag">{tag}</span>}
                  </For>
                </div>
              </Show>
              <p class="recipe-date">
                Updated {new Date(recipe.updatedAt).toLocaleDateString()}
              </p>
            </A>
          )}
        </For>
      </div>
    </div>
  );
}
