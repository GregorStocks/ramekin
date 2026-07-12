import { createSignal } from "solid-js";
import type { Accessor } from "solid-js";
import type {
  EnrichApi,
  RecipeContent,
  RecipeResponse,
  RecipesApi,
} from "ramekin-client";
import {
  runSingleRecipeAiOperation,
  type RecipeAiOperationSummary,
} from "../utils/aiEnrichments";
import { extractApiError } from "../utils/recipeFormHelpers";

interface UseRecipeAiActionsOptions {
  recipeId: Accessor<string>;
  recipe: Accessor<RecipeResponse | null>;
  getRecipesApi: () => RecipesApi;
  getEnrichApi: () => EnrichApi;
  loadRecipe: () => Promise<void>;
  clearHistoricalVersion: () => void;
  setError: (message: string | null) => void;
}

function recipeToContent(recipe: RecipeResponse): RecipeContent {
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
  const [customInstruction, setCustomInstruction] = createSignal("");
  const [showCustomEnrichInput, setShowCustomEnrichInput] = createSignal(false);
  const [generatingPhoto, setGeneratingPhoto] = createSignal(false);
  const [normalizingTitle, setNormalizingTitle] = createSignal(false);
  const [generatingDescription, setGeneratingDescription] = createSignal(false);

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

  const handleEnrich = async () => {
    const currentRecipe = options.recipe();
    if (!currentRecipe) return;

    setEnriching(true);
    options.setError(null);
    try {
      const enriched = await options.getEnrichApi().enrichRecipe({
        recipeContent: recipeToContent(currentRecipe),
      });
      setEnrichmentPreview(enriched, currentRecipe.versionId);
    } catch (err) {
      const message = await extractApiError(err, "Failed to enrich recipe");
      options.setError(message);
    } finally {
      setEnriching(false);
    }
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

  const handleNormalizeTitle = async () => {
    if (!options.recipe()) return;

    setNormalizingTitle(true);
    options.setError(null);
    try {
      const api = options.getRecipesApi();
      const summary = await runSingleRecipeAiOperation(
        options.recipeId(),
        (id) => api.normalizeTitle({ id }),
        "Failed to normalize title",
      );
      const operationError = firstOperationError(summary);
      if (operationError) {
        options.setError(operationError);
      } else if (summary.changed > 0) {
        options.clearHistoricalVersion();
        await options.loadRecipe();
      }
    } finally {
      setNormalizingTitle(false);
    }
  };

  const handleCustomEnrich = async () => {
    const currentRecipe = options.recipe();
    const instruction = customInstruction();
    if (!currentRecipe || !instruction.trim()) return;

    setEnriching(true);
    options.setError(null);
    try {
      const enriched = await options.getEnrichApi().customEnrichRecipe({
        customEnrichRequest: {
          recipe: recipeToContent(currentRecipe),
          instruction,
          photoIds:
            currentRecipe.photoIds.length > 0
              ? currentRecipe.photoIds
              : undefined,
        },
      });
      setEnrichmentPreview(enriched, currentRecipe.versionId);
      setShowCustomEnrichInput(false);
      setCustomInstruction("");
    } catch (err) {
      const message = await extractApiError(
        err,
        "Failed to apply custom enrichment",
      );
      options.setError(message);
    } finally {
      setEnriching(false);
    }
  };

  const handleGenerateDescription = async () => {
    if (!options.recipe()) return;

    setGeneratingDescription(true);
    options.setError(null);
    try {
      const api = options.getRecipesApi();
      const summary = await runSingleRecipeAiOperation(
        options.recipeId(),
        (id) => api.generateDescription({ id }),
        "Failed to generate description",
      );
      const operationError = firstOperationError(summary);
      if (operationError) {
        options.setError(operationError);
      } else if (summary.changed > 0) {
        options.clearHistoricalVersion();
        await options.loadRecipe();
      }
    } finally {
      setGeneratingDescription(false);
    }
  };

  return {
    enriching,
    enrichedContent,
    applyingEnrichment,
    customInstruction,
    setCustomInstruction,
    showCustomEnrichInput,
    setShowCustomEnrichInput,
    generatingPhoto,
    normalizingTitle,
    generatingDescription,
    handleEnrich,
    handleApplyEnrichment,
    handleGeneratePhoto,
    handleNormalizeTitle,
    handleCustomEnrich,
    handleGenerateDescription,
    handleEnrichClose: clearEnrichmentPreview,
  };
}
