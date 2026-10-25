import { createSignal, type Accessor } from "solid-js";
import type { RecipeSummary, RecipesApi } from "ramekin-client";

import {
  runRecipeAiBatchOperation,
  type RecipeAiOperationSummary,
} from "../utils/aiOperations";
import { createRequestTracker } from "../utils/requestTracker";

export type BulkOperation = "rescrapePhoto" | "photo";

export interface BulkProgress {
  operation: BulkOperation;
  done: number;
  total: number;
}

interface UseCookbookBulkSelectionOptions {
  getRecipesApi: () => RecipesApi;
  query: Accessor<string>;
  recipes: Accessor<RecipeSummary[]>;
  total: Accessor<number>;
  reloadRecipes: () => Promise<void>;
  setError: (error: string | null) => void;
  setNotice: (notice: string | null) => void;
}

export function useCookbookBulkSelection(
  options: UseCookbookBulkSelectionOptions,
) {
  const [bulkMode, setBulkMode] = createSignal(false);
  const [selected, setSelected] = createSignal<Set<string>>(new Set());
  const [selectAllStatus, setSelectAllStatus] = createSignal<string | null>(
    null,
  );
  const [bulkRecipes, setBulkRecipes] = createSignal<RecipeSummary[]>([]);
  const [showPdfModal, setShowPdfModal] = createSignal(false);
  const [showReNormalizeModal, setShowReNormalizeModal] = createSignal(false);
  const [bulkProgress, setBulkProgress] = createSignal<BulkProgress | null>(
    null,
  );
  const bulkRequests = createRequestTracker();

  const resetForQueryChange = () => {
    setSelected(new Set<string>());
    setBulkRecipes([]);
    bulkRequests.invalidate();
  };

  const toggleBulkMode = () => {
    if (bulkMode()) {
      setBulkMode(false);
      resetForQueryChange();
    } else {
      setBulkMode(true);
    }
  };

  const toggleSelected = (id: string) => {
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const fetchAllMatching = async (): Promise<RecipeSummary[] | null> => {
    const cached = bulkRecipes();
    if (cached.length > 0 && cached.length === options.total()) return cached;

    const requestId = bulkRequests.start();
    const query = options.query();
    const api = options.getRecipesApi();
    const all: RecipeSummary[] = [];
    const pageSize = 200;
    let offset = 0;

    while (true) {
      const response = await api.listRecipes({
        limit: pageSize,
        offset,
        q: query || undefined,
        sortBy: "updated_at",
        sortDir: "desc",
      });
      if (!bulkRequests.isCurrent(requestId)) return null;

      all.push(...response.recipes);
      offset += response.recipes.length;
      if (
        response.recipes.length === 0 ||
        offset >= response.pagination.total
      ) {
        break;
      }
    }

    if (!bulkRequests.isCurrent(requestId)) return null;
    setBulkRecipes(all);
    return all;
  };

  const selectAll = async () => {
    setSelectAllStatus("Loading…");
    try {
      const all = await fetchAllMatching();
      if (all === null) {
        setSelectAllStatus(null);
        return;
      }
      setSelected(new Set(all.map((recipe) => recipe.id)));
      setSelectAllStatus(null);
    } catch {
      setSelectAllStatus(null);
      options.setError("Failed to load all recipes");
    }
  };

  const clearSelection = () => setSelected(new Set<string>());

  const selectedRecipes = (): RecipeSummary[] => {
    const ids = selected();
    if (ids.size === 0) return [];
    const source = bulkRecipes().length > 0 ? bulkRecipes() : options.recipes();
    return source.filter((recipe) => ids.has(recipe.id));
  };

  /** Load every matching recipe when the selection reaches past this page. */
  const loadSelectedRecipes = async (errorMessage: string) => {
    if (
      bulkRecipes().length === 0 &&
      selected().size > options.recipes().length
    ) {
      try {
        await fetchAllMatching();
      } catch {
        options.setError(errorMessage);
        return false;
      }
    }
    return true;
  };

  const openPdfExport = async () => {
    if (await loadSelectedRecipes("Failed to load recipes for PDF")) {
      setShowPdfModal(true);
    }
  };

  const openReNormalize = async () => {
    if (await loadSelectedRecipes("Failed to load recipes to re-normalize")) {
      setShowReNormalizeModal(true);
    }
  };

  const progressFor = (operation: BulkOperation) => {
    const progress = bulkProgress();
    return progress?.operation === operation ? progress : null;
  };

  const bulkButtonLabel = (
    operation: BulkOperation,
    idleLabel: string,
    progressVerb: string,
  ) => {
    const progress = progressFor(operation);
    return progress
      ? `${progressVerb} ${progress.done}/${progress.total}…`
      : idleLabel;
  };

  const summarizeErrors = (errors: string[]) =>
    `${errors.slice(0, 3).join("; ")}${errors.length > 3 ? "…" : ""}`;

  const runSelectedBulkOperation = async <TResult>({
    operation,
    confirm,
    action,
    errorFallback,
    reloadRecipes = false,
    summarize,
  }: {
    operation: BulkOperation;
    confirm: (count: number) => string;
    action: (id: string) => Promise<TResult>;
    errorFallback: string;
    reloadRecipes?: boolean;
    summarize: (summary: RecipeAiOperationSummary) => string;
  }) => {
    const ids = Array.from(selected());
    if (ids.length === 0 || !window.confirm(confirm(ids.length))) return;

    options.setError(null);
    options.setNotice(null);
    setBulkProgress({ operation, done: 0, total: ids.length });
    const summary = await runRecipeAiBatchOperation({
      ids,
      run: action,
      errorFallback,
      onProgress: (progress) => setBulkProgress({ operation, ...progress }),
    });
    setBulkProgress(null);

    if (reloadRecipes) await options.reloadRecipes();

    const message = summarize(summary);
    if (summary.errors.length > 0) options.setError(message);
    else options.setNotice(message);
  };

  const bulkGeneratePhoto = async () => {
    const api = options.getRecipesApi();
    await runSelectedBulkOperation({
      operation: "photo",
      confirm: (count) =>
        count === 1
          ? "Generate an AI photo for this recipe?"
          : `Generate AI photos for ${count} recipes? Each one calls the image model.`,
      action: (id) => api.generatePhoto({ id }),
      errorFallback: "photo generation failed",
      reloadRecipes: true,
      summarize: (summary) =>
        summary.errors.length > 0
          ? `${summary.succeeded}/${summary.total} photos generated. Errors: ${summarizeErrors(summary.errors)}`
          : `Generated AI photos for ${summary.total} recipes.`,
    });
  };

  const bulkRescrapePhoto = async () => {
    const api = options.getRecipesApi();
    await runSelectedBulkOperation({
      operation: "rescrapePhoto",
      confirm: (count) =>
        count === 1
          ? "Queue a photo rescrape for this recipe?"
          : `Queue photo rescrapes for ${count} recipes? This will issue one job per recipe.`,
      action: (id) => api.rescrapePhoto({ id }),
      errorFallback: "rescrape failed",
      summarize: (summary) =>
        summary.errors.length > 0
          ? `${summary.succeeded}/${summary.total} jobs queued. Errors: ${summarizeErrors(summary.errors)}`
          : `Queued photo rescrapes for ${summary.total} recipes.`,
    });
  };

  return {
    bulkMode,
    selected,
    selectAllStatus,
    showPdfModal,
    setShowPdfModal,
    showReNormalizeModal,
    setShowReNormalizeModal,
    bulkProgress,
    resetForQueryChange,
    toggleBulkMode,
    toggleSelected,
    selectAll,
    clearSelection,
    selectedRecipes,
    openPdfExport,
    openReNormalize,
    bulkButtonLabel,
    bulkGeneratePhoto,
    bulkRescrapePhoto,
  };
}

export type CookbookBulkSelectionState = ReturnType<
  typeof useCookbookBulkSelection
>;
