import { describe, expect, it, vi } from "vitest";
import type { RecipeResponse, RecipesApi } from "ramekin-client";
import { useRecipeAiActions } from "./useRecipeAiActions";

function recipe(): RecipeResponse {
  return {
    id: "recipe-1",
    title: "Soup",
    description: null,
    ingredients: [],
    derivedMeasurements: [],
    instructions: "Simmer",
    photoIds: [],
    tags: [],
    createdAt: new Date("2026-01-01T00:00:00Z"),
    updatedAt: new Date("2026-01-01T00:00:00Z"),
    versionId: "version-1",
    versionSource: "user",
    normalizeOps: [],
  };
}

describe("recipe AI actions", () => {
  it("reloads the recipe after generating a photo", async () => {
    const generatePhoto = vi.fn(async () => ({}));
    const loadRecipe = vi.fn(async () => {});
    const setError = vi.fn();
    const actions = useRecipeAiActions({
      recipeId: () => "recipe-1",
      recipe: () => recipe(),
      getRecipesApi: () => ({ generatePhoto }) as unknown as RecipesApi,
      loadRecipe,
      setError,
    });

    await actions.handleGeneratePhoto();

    expect(generatePhoto).toHaveBeenCalledWith({ id: "recipe-1" });
    expect(loadRecipe).toHaveBeenCalledOnce();
    expect(actions.generatingPhoto()).toBe(false);
  });
});
