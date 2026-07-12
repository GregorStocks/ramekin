import { createEffect, onCleanup, onMount, type Accessor } from "solid-js";
import type { RecipesApi } from "ramekin-client";

import type { SortOption } from "../pages/cookbook/query";
import { createCookbookRecipeRequests } from "../pages/cookbook/recipeRequests";
import type { Density } from "../pages/cookbook/presentation";

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
  const state = createCookbookRecipeRequests(options);

  const handleScroll = () => {
    const root = document.documentElement;
    if (root.scrollHeight - root.scrollTop - root.clientHeight < 300) {
      state.loadMore();
    }
  };

  onMount(() => window.addEventListener("scroll", handleScroll));
  onCleanup(() => window.removeEventListener("scroll", handleScroll));

  createEffect(() => {
    const items = state.recipes();
    options.density();
    if (items.length > 0) requestAnimationFrame(handleScroll);
  });

  const recipeCount = () => {
    const count = state.recipes().length;
    const totalCount = state.total();
    if (totalCount === 0) return "";
    if (count < totalCount) {
      return `(showing ${count} of ${totalCount} recipes)`;
    }
    if (count === 1) return "(1 recipe)";
    return `(${count} recipes)`;
  };

  return {
    recipes: state.recipes,
    loading: state.loading,
    loadingMore: state.loadingMore,
    total: state.total,
    hasMore: state.hasMore,
    loadRecipes: state.loadRecipes,
    resetAndLoad: state.resetAndLoad,
    recipeCount,
  };
}

export type CookbookRecipesState = ReturnType<typeof useCookbookRecipes>;
