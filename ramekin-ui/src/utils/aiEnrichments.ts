import { extractApiError } from "./recipeFormHelpers";

export const AI_ENRICHMENTS = {
  normalizeTitle: {
    bulkLabel: "Auto-rename",
    individualLabel: "Auto-rename",
    progressVerb: "Renaming",
  },
  generateDescription: {
    bulkLabel: "Generate descriptions",
    individualLabel: "Generate Description",
    progressVerb: "Describing",
  },
  generatePhoto: {
    bulkLabel: "Generate AI photos",
    individualLabel: "Generate AI Photo",
    progressVerb: "Generating photos",
  },
  enrichRecipe: {
    individualLabel: "Enrich with AI",
  },
  customEnrich: {
    individualLabel: "Customize with AI",
  },
} as const;

export interface RecipeAiOperationProgress {
  done: number;
  total: number;
}

export interface RecipeAiOperationSummary {
  total: number;
  succeeded: number;
  changed: number;
  errors: string[];
}

interface RecipeAiBatchOperationOptions<T> {
  ids: string[];
  run: (id: string) => Promise<T>;
  errorFallback: string;
  onProgress?: (progress: RecipeAiOperationProgress) => void;
  formatError?: (id: string, message: string) => string;
}

function changedFromResult(result: unknown): boolean {
  return (
    result !== null &&
    typeof result === "object" &&
    "changed" in result &&
    result.changed === true
  );
}

export async function runRecipeAiBatchOperation<T>({
  ids,
  run,
  errorFallback,
  onProgress,
  formatError = (id, message) => `${id.slice(0, 8)}: ${message}`,
}: RecipeAiBatchOperationOptions<T>): Promise<RecipeAiOperationSummary> {
  let done = 0;
  let changed = 0;
  const errors: string[] = [];

  for (const id of ids) {
    try {
      const result = await run(id);
      if (changedFromResult(result)) changed += 1;
    } catch (err) {
      const message = await extractApiError(err, errorFallback);
      errors.push(formatError(id, message));
    }

    done += 1;
    onProgress?.({ done, total: ids.length });
  }

  return {
    total: ids.length,
    succeeded: ids.length - errors.length,
    changed,
    errors,
  };
}

export async function runSingleRecipeAiOperation<T>(
  id: string,
  run: (id: string) => Promise<T>,
  errorFallback: string,
): Promise<RecipeAiOperationSummary> {
  return runRecipeAiBatchOperation({
    ids: [id],
    run,
    errorFallback,
    formatError: (_id, message) => message,
  });
}
