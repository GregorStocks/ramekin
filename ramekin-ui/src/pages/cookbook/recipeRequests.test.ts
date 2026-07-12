import { describe, expect, it } from "vitest";
import { createSignal } from "solid-js";
import type {
  ListRecipesRequest,
  ListRecipesResponse,
  RecipeSummary,
} from "ramekin-client";

import {
  createCookbookRecipeRequests,
  type CookbookRecipesApi,
} from "./recipeRequests";
import type { SortOption } from "./query";

interface Deferred {
  request: ListRecipesRequest;
  resolve: (response: ListRecipesResponse) => void;
  reject: (reason: unknown) => void;
}

/**
 * A listRecipes fake that never settles on its own: each call parks a deferred
 * the test resolves or rejects in whatever order it wants to simulate.
 */
function createDeferredApi() {
  const pending: Deferred[] = [];

  const api: CookbookRecipesApi = {
    listRecipes: (request) =>
      new Promise<ListRecipesResponse>((resolve, reject) => {
        pending.push({ request, resolve, reject });
      }),
  };

  return { api, pending };
}

function recipe(id: string): RecipeSummary {
  return {
    createdAt: new Date(0),
    id,
    tags: [],
    title: id,
    updatedAt: new Date(0),
  };
}

function page(ids: string[], total: number, offset = 0): ListRecipesResponse {
  return {
    pagination: { limit: 20, offset, total },
    recipes: ids.map(recipe),
  };
}

function setup() {
  const { api, pending } = createDeferredApi();
  const [query, setQuery] = createSignal("");
  const [sortOption, setSortOption] = createSignal<SortOption>("newest");
  let error: string | null = null;
  let notice: string | null = null;

  const state = createCookbookRecipeRequests({
    getRecipesApi: () => api,
    query,
    sortOption,
    hasTextQuery: () => query().length > 0,
    setError: (value) => {
      error = value;
    },
    setNotice: (value) => {
      notice = value;
    },
  });

  return {
    state,
    pending,
    setQuery,
    setSortOption,
    getError: () => error,
    getNotice: () => notice,
  };
}

/** Let already-resolved promise callbacks run. */
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("createCookbookRecipeRequests", () => {
  it("ignores an older initial response that resolves after a newer one", async () => {
    const { state, pending, setQuery, getError } = setup();

    state.resetAndLoad();
    setQuery("cake");
    state.resetAndLoad();
    expect(pending).toHaveLength(2);

    pending[1].resolve(page(["cake-1"], 1));
    await flush();
    pending[0].resolve(page(["stale-1", "stale-2"], 40));
    await flush();

    expect(state.recipes().map((item) => item.id)).toEqual(["cake-1"]);
    expect(state.total()).toBe(1);
    expect(state.offset()).toBe(1);
    expect(state.hasMore()).toBe(false);
    expect(state.loading()).toBe(false);
    expect(getError()).toBeNull();
  });

  it("ignores an older initial failure that rejects after a newer response", async () => {
    const { state, pending, setQuery, getError } = setup();

    state.resetAndLoad();
    setQuery("cake");
    state.resetAndLoad();

    pending[1].resolve(page(["cake-1"], 1));
    await flush();
    pending[0].reject(new Error("slow network"));
    await flush();

    expect(state.recipes().map((item) => item.id)).toEqual(["cake-1"]);
    expect(getError()).toBeNull();
    expect(state.loading()).toBe(false);
  });

  it("keeps loading until the current request settles", async () => {
    const { state, pending } = setup();

    state.resetAndLoad();
    state.resetAndLoad();

    pending[0].resolve(page(["stale-1"], 1));
    await flush();
    expect(state.loading()).toBe(true);

    pending[1].resolve(page(["fresh-1"], 1));
    await flush();
    expect(state.loading()).toBe(false);
    expect(state.recipes().map((item) => item.id)).toEqual(["fresh-1"]);
  });

  it("discards an in-flight append when the query changes", async () => {
    const { state, pending, setQuery } = setup();

    state.resetAndLoad();
    pending[0].resolve(page(["a-1", "a-2"], 40));
    await flush();

    state.loadMore();
    expect(pending[1].request.offset).toBe(2);
    expect(state.loadingMore()).toBe(true);

    setQuery("cake");
    state.resetAndLoad();

    pending[1].resolve(page(["a-3", "a-4"], 40, 2));
    await flush();
    pending[2].resolve(page(["cake-1"], 1));
    await flush();

    expect(state.recipes().map((item) => item.id)).toEqual(["cake-1"]);
    expect(state.total()).toBe(1);
    expect(state.offset()).toBe(1);
    expect(state.hasMore()).toBe(false);
    expect(state.loadingMore()).toBe(false);
  });

  it("discards an in-flight append failure when the sort changes", async () => {
    const { state, pending, setSortOption, getError } = setup();

    state.resetAndLoad();
    pending[0].resolve(page(["a-1", "a-2"], 40));
    await flush();

    state.loadMore();
    setSortOption("title");
    state.resetAndLoad();

    pending[1].reject(new Error("slow network"));
    await flush();
    pending[2].resolve(page(["t-1"], 1));
    await flush();

    expect(pending[2].request.sortBy).toBe("title");
    expect(state.recipes().map((item) => item.id)).toEqual(["t-1"]);
    expect(getError()).toBeNull();
    expect(state.loadingMore()).toBe(false);
  });

  it("discards an append that lands before the query change triggers its reload", async () => {
    const { state, pending, setQuery } = setup();

    state.resetAndLoad();
    pending[0].resolve(page(["a-1", "a-2"], 40));
    await flush();

    state.loadMore();

    // The query changed but nothing has called resetAndLoad yet — on iOS the
    // reload is debounced, so this window is wide.
    setQuery("cake");
    pending[1].resolve(page(["a-3", "a-4"], 40, 2));
    await flush();

    expect(state.recipes().map((item) => item.id)).toEqual(["a-1", "a-2"]);
    expect(state.offset()).toBe(2);
    // Discarding the append must not strand the spinner, or a stuck
    // loadingMore blocks every later page.
    expect(state.loadingMore()).toBe(false);

    // Back on the original query, pagination still works.
    setQuery("");
    state.loadMore();
    pending[2].resolve(page(["a-3"], 3, 2));
    await flush();

    expect(state.recipes().map((item) => item.id)).toEqual([
      "a-1",
      "a-2",
      "a-3",
    ]);
  });

  it("does not start an append once the query changed but the reload has not run", async () => {
    const { state, pending, setQuery } = setup();

    state.resetAndLoad();
    pending[0].resolve(page(["a-1", "a-2"], 40));
    await flush();

    setQuery("cake");
    state.loadMore();

    expect(pending).toHaveLength(1);
  });

  it("does not start an append while an initial load is in flight", async () => {
    const { state, pending } = setup();

    state.resetAndLoad();
    state.loadMore();

    expect(pending).toHaveLength(1);
  });

  it("appends the next page when nothing superseded the request", async () => {
    const { state, pending, getNotice } = setup();

    state.resetAndLoad();
    pending[0].resolve(page(["a-1", "a-2"], 3));
    await flush();

    state.loadMore();
    pending[1].resolve(page(["a-3"], 3, 2));
    await flush();

    expect(state.recipes().map((item) => item.id)).toEqual([
      "a-1",
      "a-2",
      "a-3",
    ]);
    expect(state.offset()).toBe(3);
    expect(state.hasMore()).toBe(false);
    expect(state.loadingMore()).toBe(false);
    expect(getNotice()).toBeNull();
  });
});
