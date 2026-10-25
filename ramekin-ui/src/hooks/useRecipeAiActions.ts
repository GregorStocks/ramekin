import { createSignal } from "solid-js";
import type { Accessor } from "solid-js";
import type { RecipeResponse, RecipesApi } from "ramekin-client";
import {
  runSingleRecipeAiOperation,
  type RecipeAiOperationSummary,
} from "../utils/aiOperations";

interface UseRecipeAiActionsOptions {
  recipeId: Accessor<string>;
  recipe: Accessor<RecipeResponse | null>;
  getRecipesApi: () => RecipesApi;
  loadRecipe: () => Promise<void>;
  setError: (message: string | null) => void;
}

function firstOperationError(summary: RecipeAiOperationSummary): string | null {
  return summary.errors.length > 0 ? summary.errors[0] : null;
}

/** AI actions on a recipe page that save directly: generating a photo. */
export function useRecipeAiActions(options: UseRecipeAiActionsOptions) {
  const [generatingPhoto, setGeneratingPhoto] = createSignal(false);

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

  return {
    generatingPhoto,
    handleGeneratePhoto,
  };
}
