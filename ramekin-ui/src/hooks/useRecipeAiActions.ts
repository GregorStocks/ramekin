import { createSignal } from "solid-js";
import type { Accessor } from "solid-js";
import type {
  EditRecipeApi,
  RecipeContent,
  RecipeResponse,
  RecipesApi,
} from "ramekin-client";
import {
  runSingleRecipeAiOperation,
  type RecipeAiOperationSummary,
} from "../utils/aiOperations";
import { extractApiError } from "../utils/recipeFormHelpers";

interface UseRecipeAiActionsOptions {
  recipeId: Accessor<string>;
  recipe: Accessor<RecipeResponse | null>;
  getRecipesApi: () => RecipesApi;
  getEditRecipeApi: () => EditRecipeApi;
  loadRecipe: () => Promise<void>;
  setError: (message: string | null) => void;
}

export function recipeToContent(recipe: RecipeResponse): RecipeContent {
  return {
    title: recipe.title,
    description: recipe.description,
    instructions: recipe.instructions,
    ingredients: recipe.ingredients,
    tags: recipe.tags,
    prepTime: recipe.prepTime,
    cookTime: recipe.cookTime,
    totalTime: recipe.totalTime,
    servings: recipe.servings,
    difficulty: recipe.difficulty,
    notes: recipe.notes,
    nutritionalInfo: recipe.nutritionalInfo,
    sourceName: recipe.sourceName,
    sourceUrl: recipe.sourceUrl,
  };
}

function firstOperationError(summary: RecipeAiOperationSummary): string | null {
  return summary.errors.length > 0 ? summary.errors[0] : null;
}

export function useRecipeAiActions(options: UseRecipeAiActionsOptions) {
  const [enriching, setEnriching] = createSignal(false);
  const [enrichedContent, setEnrichedContent] =
    createSignal<RecipeContent | null>(null);
  const [enrichmentSourceVersionId, setEnrichmentSourceVersionId] =
    createSignal<string | null>(null);
  const [applyingEnrichment, setApplyingEnrichment] = createSignal(false);
  const [editInstruction, setEditInstruction] = createSignal("");
  const [showEditWithAiInput, setShowEditWithAiInput] = createSignal(false);
  const [generatingPhoto, setGeneratingPhoto] = createSignal(false);

  const setEnrichmentPreview = (
    content: RecipeContent,
    sourceVersionId: string,
  ) => {
    setEnrichedContent(content);
    setEnrichmentSourceVersionId(sourceVersionId);
  };

  const clearEnrichmentPreview = () => {
    setEnrichedContent(null);
    setEnrichmentSourceVersionId(null);
  };

  const handleApplyEnrichment = async () => {
    const enriched = enrichedContent();
    if (!enriched) return;
    const expectedVersionId = enrichmentSourceVersionId();
    if (!expectedVersionId) {
      throw new Error(
        "Cannot apply enrichment without its source recipe version",
      );
    }

    setApplyingEnrichment(true);
    try {
      await options.getRecipesApi().updateRecipe({
        id: options.recipeId(),
        updateRecipeRequest: {
          expectedVersionId,
          title: enriched.title,
          description: enriched.description,
          instructions: enriched.instructions,
          ingredients: enriched.ingredients,
          tags: enriched.tags,
          prepTime: enriched.prepTime,
          cookTime: enriched.cookTime,
          totalTime: enriched.totalTime,
          servings: enriched.servings,
          difficulty: enriched.difficulty,
          notes: enriched.notes,
          nutritionalInfo: enriched.nutritionalInfo,
          sourceName: enriched.sourceName,
          sourceUrl: enriched.sourceUrl,
        },
      });
      clearEnrichmentPreview();
      await options.loadRecipe();
    } catch (err) {
      const message = await extractApiError(err, "Failed to apply enrichment");
      options.setError(message);
    } finally {
      setApplyingEnrichment(false);
    }
  };

  const handleGeneratePhoto = async () => {
    if (!options.recipe()) return;

    setGeneratingPhoto(true);
    options.setError(null);
    try {
      const api = options.getRecipesApi();
      const summary = await runSingleRecipeAiOperation(
        options.recipeId(),
        (id) => api.generatePhoto({ id }),
        "Failed to generate AI photo",
      );
      const operationError = firstOperationError(summary);
      if (operationError) {
        options.setError(operationError);
      } else {
        await options.loadRecipe();
      }
    } finally {
      setGeneratingPhoto(false);
    }
  };

  const handleEditWithAi = async () => {
    const currentRecipe = options.recipe();
    const instruction = editInstruction();
    if (!currentRecipe || !instruction.trim()) return;

    setEnriching(true);
    options.setError(null);
    try {
      const edited = await options.getEditRecipeApi().editRecipeWithAi({
        editRecipeRequest: {
          recipe: recipeToContent(currentRecipe),
          instruction,
          photoIds:
            currentRecipe.photoIds.length > 0
              ? currentRecipe.photoIds
              : undefined,
        },
      });
      setEnrichmentPreview(edited, currentRecipe.versionId);
      setShowEditWithAiInput(false);
      setEditInstruction("");
    } catch (err) {
      const message = await extractApiError(err, "Failed to edit with AI");
      options.setError(message);
    } finally {
      setEnriching(false);
    }
  };

  return {
    enriching,
    enrichedContent,
    applyingEnrichment,
    editInstruction,
    setEditInstruction,
    showEditWithAiInput,
    setShowEditWithAiInput,
    generatingPhoto,
    handleApplyEnrichment,
    handleGeneratePhoto,
    handleEditWithAi,
    handleEnrichClose: clearEnrichmentPreview,
  };
}
