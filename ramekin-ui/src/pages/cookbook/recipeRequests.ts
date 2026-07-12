import { createSignal, type Accessor } from "solid-js";
import type {
  ListRecipesRequest,
  ListRecipesResponse,
  RecipeSummary,
} from "ramekin-client";

import { createRequestTracker } from "../../utils/requestTracker";
import { getSortParams, type SortOption } from "./query";

export const COOKBOOK_PAGE_SIZE = 20;

/** The slice of RecipesApi the cookbook list needs, so tests can fake it. */
export interface CookbookRecipesApi {
  listRecipes: (request: ListRecipesRequest) => Promise<ListRecipesResponse>;
}

export interface CookbookRecipeRequestsOptions {
  getRecipesApi: () => CookbookRecipesApi;
  query: Accessor<string>;
  sortOption: Accessor<SortOption>;
  hasTextQuery: Accessor<boolean>;
  setError: (error: string | null) => void;
  setNotice: (notice: string | null) => void;
}

/**
 * Recipe list state for the cookbook, isolated from the page's DOM wiring so it
 * can be unit-tested. Every request carries an id and only the current request
 * may write recipes, pagination, error, or loading state — a filter or sort
 * change supersedes whatever initial or append request was in flight.
 */
export function createCookbookRecipeRequests(
  options: CookbookRecipeRequestsOptions,
) {
  const [recipes, setRecipes] = createSignal<RecipeSummary[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [loadingMore, setLoadingMore] = createSignal(false);
  const [offset, setOffset] = createSignal(0);
  const [total, setTotal] = createSignal(0);
  const [hasMore, setHasMore] = createSignal(true);
  const requests = createRequestTracker();

  const loadRecipes = async (appendMode = false, currentOffset = 0) => {
    const requestId = requests.start();

    if (appendMode) {
      setLoadingMore(true);
    } else {
      setLoading(true);
      setLoadingMore(false);
    }
    options.setError(null);
    if (!appendMode) options.setNotice(null);

    try {
      const query = options.query();
      const { sortBy, sortDir } = getSortParams(
        options.hasTextQuery() ? "best" : options.sortOption(),
      );
      const response = await options.getRecipesApi().listRecipes({
        limit: COOKBOOK_PAGE_SIZE,
        offset: currentOffset,
        q: query || undefined,
        sortBy,
        sortDir,
      });
      if (!requests.isCurrent(requestId)) return;

      const loaded = currentOffset + response.recipes.length;
      setRecipes(
        appendMode ? [...recipes(), ...response.recipes] : response.recipes,
      );
      setTotal(response.pagination.total);
      setOffset(loaded);
      setHasMore(loaded < response.pagination.total);
    } catch {
      if (!requests.isCurrent(requestId)) return;
      options.setError("Failed to load recipes");
    } finally {
      if (requests.isCurrent(requestId)) {
        setLoading(false);
        setLoadingMore(false);
      }
    }
  };

  const resetAndLoad = () => {
    setOffset(0);
    setRecipes([]);
    void loadRecipes(false, 0);
  };

  const loadMore = () => {
    if (loading() || loadingMore() || !hasMore()) return;
    void loadRecipes(true, offset());
  };

  return {
    recipes,
    loading,
    loadingMore,
    offset,
    total,
    hasMore,
    loadRecipes,
    resetAndLoad,
    loadMore,
  };
}

export type CookbookRecipeRequests = ReturnType<
  typeof createCookbookRecipeRequests
>;
