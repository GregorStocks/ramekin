import { describe, expect, it, vi } from "vitest";
import type {
  EnrichApi,
  RecipeContent,
  RecipeResponse,
  RecipesApi,
  UpdateRecipeRequest,
} from "ramekin-client";
import { useRecipeAiActions } from "./useRecipeAiActions";

function recipe(versionId: string): RecipeResponse {
  return {
    id: "recipe-1",
    title: "Soup",
    description: null,
    ingredients: [],
    instructions: "Simmer",
    photoIds: [],
    tags: [],
    createdAt: new Date("2026-01-01T00:00:00Z"),
    updatedAt: new Date("2026-01-01T00:00:00Z"),
    versionId,
    versionSource: "user",
  };
}

describe("recipe AI actions", () => {
  it("applies an enrichment against the version that produced its preview", async () => {
    let currentRecipe = recipe("version-1");
    let submittedRequest: UpdateRecipeRequest | undefined;
    const enriched: RecipeContent = {
      title: "Better Soup",
      ingredients: [],
      instructions: "Simmer gently",
      tags: [],
    };
    const actions = useRecipeAiActions({
      recipeId: () => currentRecipe.id,
      recipe: () => currentRecipe,
      getRecipesApi: () =>
        ({
          updateRecipe: vi.fn(async ({ updateRecipeRequest }) => {
            submittedRequest = updateRecipeRequest;
          }),
        }) as unknown as RecipesApi,
      getEnrichApi: () =>
        ({
          enrichRecipe: vi.fn(async () => enriched),
        }) as unknown as EnrichApi,
      loadRecipe: vi.fn(async () => {}),
      clearHistoricalVersion: vi.fn(),
      setError: vi.fn(),
    });

    await actions.handleEnrich();
    currentRecipe = recipe("version-2");
    await actions.handleApplyEnrichment();

    expect(submittedRequest?.expectedVersionId).toBe("version-1");
  });
});
