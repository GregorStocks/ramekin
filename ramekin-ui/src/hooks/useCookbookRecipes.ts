import {
  createEffect,
  createSignal,
  onCleanup,
  onMount,
  type Accessor,
} from "solid-js";
import type { RecipeSummary, RecipesApi } from "ramekin-client";

import { getSortParams, type SortOption } from "../pages/cookbook/query";
import type { Density } from "../pages/cookbook/presentation";

const PAGE_SIZE = 20;

interface UseCookbookRecipesOptions {
  getRecipesApi: () => RecipesApi;
  query: Accessor<string>;
  sortOption: Accessor<SortOption>;
  hasTextQuery: Accessor<boolean>;
  density: Accessor<Density>;
  setError: (error: string | null) => void;
  setNotice: (notice: string | null) => void;
}

export function useCookbookRecipes(options: UseCookbookRecipesOptions) {
  const [recipes, setRecipes] = createSignal<RecipeSummary[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [loadingMore, setLoadingMore] = createSignal(false);
  const [offset, setOffset] = createSignal(0);
  const [total, setTotal] = createSignal(0);
  const [hasMore, setHasMore] = createSignal(true);

  const loadRecipes = async (appendMode = false, currentOffset = 0) => {
    if (appendMode) {
      setLoadingMore(true);
    } else {
      setLoading(true);
    }
    options.setError(null);
    if (!appendMode) options.setNotice(null);

    try {
      const query = options.query();
      const { sortBy, sortDir } = getSortParams(
        options.hasTextQuery() ? "best" : options.sortOption(),
      );
      const response = await options.getRecipesApi().listRecipes({
        limit: PAGE_SIZE,
        offset: currentOffset,
        q: query || undefined,
        sortBy,
        sortDir,
      });

      setRecipes(
        appendMode ? [...recipes(), ...response.recipes] : response.recipes,
      );
      setTotal(response.pagination.total);
      setOffset(currentOffset + response.recipes.length);
      setHasMore(
        currentOffset + response.recipes.length < response.pagination.total,
      );
    } catch {
      options.setError("Failed to load recipes");
    } finally {
      setLoading(false);
      setLoadingMore(false);
    }
  };

  const resetAndLoad = () => {
    setOffset(0);
    setRecipes([]);
    void loadRecipes(false, 0);
  };

  const loadMore = () => {
    if (!loadingMore() && hasMore()) {
      void loadRecipes(true, offset());
    }
  };

  const handleScroll = () => {
    const root = document.documentElement;
    if (root.scrollHeight - root.scrollTop - root.clientHeight < 300) {
      loadMore();
    }
  };

  onMount(() => window.addEventListener("scroll", handleScroll));
  onCleanup(() => window.removeEventListener("scroll", handleScroll));

  createEffect(() => {
    const items = recipes();
    options.density();
    if (items.length > 0) requestAnimationFrame(handleScroll);
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

  return {
    recipes,
    loading,
    loadingMore,
    total,
    hasMore,
    loadRecipes,
    resetAndLoad,
    recipeCount,
  };
}

export type CookbookRecipesState = ReturnType<typeof useCookbookRecipes>;
