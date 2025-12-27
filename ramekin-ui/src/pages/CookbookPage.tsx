import { createSignal, Show, For, onMount, onCleanup } from "solid-js";
import { A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import type { RecipeSummary } from "ramekin-client";

function PhotoThumbnail(props: {
  photoId: string;
  token: string;
  alt: string;
}) {
  const [src, setSrc] = createSignal<string | null>(null);

  onMount(async () => {
    const response = await fetch(`/api/photos/${props.photoId}/thumbnail`, {
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
    <Show when={src()} fallback={<div class="recipe-card-placeholder">🍽️</div>}>
      <img src={src()!} alt={props.alt} class="recipe-card-thumbnail" />
    </Show>
  );
}

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
  const { getRecipesApi, token } = useAuth();

  const [recipes, setRecipes] = createSignal<RecipeSummary[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [loadingMore, setLoadingMore] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [offset, setOffset] = createSignal(0);
  const [total, setTotal] = createSignal(0);
  const [hasMore, setHasMore] = createSignal(true);

  const PAGE_SIZE = 20;

  const loadRecipes = async (appendMode = false) => {
    if (appendMode) {
      setLoadingMore(true);
    } else {
      setLoading(true);
    }
    setError(null);

    try {
      const response = await getRecipesApi().listRecipes({
        limit: PAGE_SIZE,
        offset: offset(),
      });

      if (appendMode) {
        setRecipes([...recipes(), ...response.recipes]);
      } else {
        setRecipes(response.recipes);
      }

      setTotal(response.pagination.total);
      setOffset(offset() + response.recipes.length);
      setHasMore(
        offset() + response.recipes.length < response.pagination.total,
      );
    } catch (err) {
      setError("Failed to load recipes");
    } finally {
      setLoading(false);
      setLoadingMore(false);
    }
  };

  const loadMore = () => {
    if (!loadingMore() && hasMore()) {
      loadRecipes(true);
    }
  };

  // Scroll listener for infinite scroll
  const handleScroll = () => {
    const scrollHeight = document.documentElement.scrollHeight;
    const scrollTop = document.documentElement.scrollTop;
    const clientHeight = document.documentElement.clientHeight;

    // Load more when user is within 300px of bottom
    if (scrollHeight - scrollTop - clientHeight < 300) {
      loadMore();
    }
  };

  onMount(() => {
    loadRecipes();
    window.addEventListener("scroll", handleScroll);
  });

  onCleanup(() => {
    window.removeEventListener("scroll", handleScroll);
  });

  const recipeCount = () => {
    const count = recipes().length;
    const totalCount = total();
    if (totalCount === 0) return "";
    if (count < totalCount) {
      return `(showing ${count} of ${totalCount} recipes)`;
    }
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
                  when={recipe.thumbnailPhotoId}
                  fallback={<div class="recipe-card-placeholder">🍽️</div>}
                >
                  <PhotoThumbnail
                    photoId={recipe.thumbnailPhotoId!}
                    token={token()!}
                    alt=""
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

        <Show when={loadingMore()}>
          <p
            class="loading"
            style={{ "text-align": "center", padding: "2rem" }}
          >
            Loading more recipes...
          </p>
        </Show>

        <Show when={!loadingMore() && !hasMore()}>
          <p
            class="loading"
            style={{ "text-align": "center", padding: "2rem", color: "#666" }}
          >
            No more recipes
          </p>
        </Show>
      </Show>
    </div>
  );
}
