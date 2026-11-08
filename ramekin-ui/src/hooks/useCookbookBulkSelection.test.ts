import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  ListRecipesResponse,
  RecipeSummary,
  RecipesApi,
} from "ramekin-client";
import { useCookbookBulkSelection } from "./useCookbookBulkSelection";

function recipe(id: string): RecipeSummary {
  return {
    id,
    title: id,
    tags: [],
    createdAt: new Date("2026-01-01T00:00:00Z"),
    updatedAt: new Date("2026-01-01T00:00:00Z"),
  } as RecipeSummary;
}

function setup(api: Partial<RecipesApi>) {
  return useCookbookBulkSelection({
    getRecipesApi: () => api as RecipesApi,
    query: () => "",
    recipes: () => [recipe("a")],
    total: () => 2,
    reloadRecipes: async () => {},
  });
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("useCookbookBulkSelection", () => {
  it("tracks select-all as a flag, not a label", async () => {
    let resolveList!: (value: ListRecipesResponse) => void;
    const listRecipes = vi.fn(
      () =>
        new Promise<ListRecipesResponse>((resolve) => {
          resolveList = resolve;
        }),
    );
    const state = setup({ listRecipes });

    const selecting = state.selectAll();
    expect(state.selectingAll()).toBe(true);
    // A second click while loading starts nothing.
    void state.selectAll();
    expect(listRecipes).toHaveBeenCalledOnce();

    resolveList({
      pagination: { limit: 200, offset: 0, total: 2 },
      recipes: [recipe("a"), recipe("b")],
    });
    await selecting;

    expect(state.selectingAll()).toBe(false);
    expect([...state.selected()].sort()).toEqual(["a", "b"]);
  });

  it("reports a failed select-all as its own result", async () => {
    const state = setup({
      listRecipes: vi.fn(async () => {
        throw new Error("offline");
      }),
    });

    await state.selectAll();

    expect(state.selectingAll()).toBe(false);
    expect(state.result()).toEqual({
      kind: "error",
      message: "Failed to load all recipes",
    });
  });

  it("keeps a bulk action's result in its own state", async () => {
    vi.stubGlobal("window", { confirm: () => true });
    const rescrapePhoto = vi.fn(async () => ({}));
    const state = setup({ rescrapePhoto } as unknown as Partial<RecipesApi>);
    state.toggleBulkMode();
    state.toggleSelected("a");

    await state.bulkRescrapePhoto();

    expect(rescrapePhoto).toHaveBeenCalledOnce();
    expect(state.result()?.kind).toBe("notice");
  });

  it("reports failed bulk actions as an error result", async () => {
    vi.stubGlobal("window", { confirm: () => true });
    const rescrapePhoto = vi.fn(async () => {
      throw new Error("nope");
    });
    const state = setup({ rescrapePhoto } as unknown as Partial<RecipesApi>);
    state.toggleSelected("a");

    await state.bulkRescrapePhoto();

    expect(state.result()?.kind).toBe("error");
  });
});
