import { describe, expect, it, vi } from "vitest";
import type {
  EditRecipeApi,
  EditRecipeWithAiRequest,
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
    derivedMeasurements: [],
    instructions: "Simmer",
    photoIds: [],
    tags: [],
    createdAt: new Date("2026-01-01T00:00:00Z"),
    updatedAt: new Date("2026-01-01T00:00:00Z"),
    versionId,
    versionSource: "user",
    normalizeOps: [],
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
      getEditRecipeApi: () => ({}) as unknown as EditRecipeApi,
      loadRecipe: vi.fn(async () => {}),
      clearHistoricalVersion: vi.fn(),
      setError: vi.fn(),
    });

    await actions.handleEnrich();
    currentRecipe = recipe("version-2");
    await actions.handleApplyEnrichment();

    expect(submittedRequest?.expectedVersionId).toBe("version-1");
  });

  it("previews an AI edit of the current recipe with the user's instruction", async () => {
    const current = recipe("version-1");
    const edited: RecipeContent = {
      title: "Vegan Soup",
      ingredients: [],
      instructions: "Simmer",
      tags: [],
    };
    let sentRequest: EditRecipeWithAiRequest | undefined;
    const actions = useRecipeAiActions({
      recipeId: () => current.id,
      recipe: () => current,
      getRecipesApi: () => ({}) as unknown as RecipesApi,
      getEnrichApi: () => ({}) as unknown as EnrichApi,
      getEditRecipeApi: () =>
        ({
          editRecipeWithAi: vi.fn(async (request: EditRecipeWithAiRequest) => {
            sentRequest = request;
            return edited;
          }),
        }) as unknown as EditRecipeApi,
      loadRecipe: vi.fn(async () => {}),
      clearHistoricalVersion: vi.fn(),
      setError: vi.fn(),
    });

    actions.setShowEditWithAiInput(true);
    actions.setEditInstruction("make it vegan");
    await actions.handleEditWithAi();

    expect(sentRequest?.editRecipeRequest.instruction).toBe("make it vegan");
    expect(sentRequest?.editRecipeRequest.recipe.title).toBe("Soup");
    expect(actions.enrichedContent()).toEqual(edited);
    expect(actions.showEditWithAiInput()).toBe(false);
    expect(actions.editInstruction()).toBe("");
  });
});
