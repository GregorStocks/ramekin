import { createEffect, createSignal, Show } from "solid-js";
import { useNavigate, useSearchParams } from "@solidjs/router";

import CookbookBulkToolbar from "../components/cookbook/CookbookBulkToolbar";
import CookbookFilters from "../components/cookbook/CookbookFilters";
import CookbookRecipeGrid from "../components/cookbook/CookbookRecipeGrid";
import CookbookToolbar from "../components/cookbook/CookbookToolbar";
import PdfExportModal from "../components/PdfExportModal";
import { useAuth } from "../context/AuthContext";
import { useCookbookBulkSelection } from "../hooks/useCookbookBulkSelection";
import { useCookbookFilters } from "../hooks/useCookbookFilters";
import { useCookbookRecipes } from "../hooks/useCookbookRecipes";
import {
  getQueryParam,
  parseSortOption,
  type SortOption,
} from "./cookbook/query";
import {
  loadDensity,
  saveDensity,
  type Density,
} from "./cookbook/presentation";
import { extractApiError } from "../utils/recipeFormHelpers";
import { usePageTitle } from "../utils/pageTitle";

export default function CookbookPage() {
  usePageTitle(() => "Cookbook");
  const { getRecipesApi, tags: availableTags, token, authedFetch } = useAuth();
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();

  const searchQuery = () => getQueryParam(searchParams.q);
  const sortOption = () => parseSortOption(getQueryParam(searchParams.sort));

  const [searchInput, setSearchInput] = createSignal(searchQuery());
  const [density, setDensity] = createSignal<Density>(loadDensity());
  const [error, setError] = createSignal<string | null>(null);
  const [notice, setNotice] = createSignal<string | null>(null);

  createEffect(() => saveDensity(density()));

  const setQuery = (query: string) => {
    setSearchInput(query);
    setSearchParams({ q: query || undefined });
  };

  const filters = useCookbookFilters({
    query: searchQuery,
    availableTags,
    setQuery,
  });

  const recipeList = useCookbookRecipes({
    getRecipesApi,
    query: searchQuery,
    sortOption,
    hasTextQuery: filters.hasTextQuery,
    density,
    setError,
    setNotice,
  });

  const bulkSelection = useCookbookBulkSelection({
    getRecipesApi,
    query: searchQuery,
    recipes: recipeList.recipes,
    total: recipeList.total,
    reloadRecipes: () => recipeList.loadRecipes(),
    setError,
    setNotice,
  });

  createEffect(() => {
    const query = searchQuery();
    sortOption();
    setSearchInput(query);
    bulkSelection.resetForQueryChange();
    recipeList.resetAndLoad();
  });

  const updateSearchQuery = (value: string) => {
    setSearchInput(value);
    const query = value.trim();
    if (query !== searchQuery()) {
      setSearchParams({ q: query || undefined });
    }
  };

  const handleSearch = (event?: Event) => {
    event?.preventDefault();
    updateSearchQuery(searchInput());
  };

  const clearSearch = () => {
    setSearchInput("");
    setSearchParams({ q: undefined });
  };

  const setSortOption = (sort: SortOption) => {
    setSearchParams({ sort: sort === "newest" ? undefined : sort });
  };

  const goToRandomRecipe = async () => {
    try {
      const query = searchQuery();
      const response = await getRecipesApi().listRecipes({
        q: query || undefined,
        limit: 1,
        sortBy: "random",
      });
      if (response.recipes.length > 0) {
        navigate(
          `/recipes/${response.recipes[0].id}?randomQ=${encodeURIComponent(query)}`,
        );
      }
    } catch (caught) {
      setError(await extractApiError(caught, "Failed to load random recipe"));
    }
  };

  return (
    <div class="cookbook-page">
      <div class="page-header">
        <h2>
          My Cookbook{" "}
          <Show when={!recipeList.loading() && recipeList.total() > 0}>
            <span class="recipe-count">{recipeList.recipeCount()}</span>
          </Show>
        </h2>
      </div>

      <CookbookToolbar
        searchInput={searchInput}
        updateSearchQuery={updateSearchQuery}
        handleSearch={handleSearch}
        clearSearch={clearSearch}
        activeFilterCount={filters.activeFilterCount}
        openFilters={() => filters.setMobileFiltersOpen(true)}
        hasTextQuery={filters.hasTextQuery}
        sortOption={sortOption}
        setSortOption={setSortOption}
        goToRandomRecipe={goToRandomRecipe}
        total={recipeList.total}
        bulkMode={bulkSelection.bulkMode}
        toggleBulkMode={bulkSelection.toggleBulkMode}
        density={density}
        setDensity={setDensity}
      />

      <CookbookBulkToolbar state={bulkSelection} total={recipeList.total} />

      <div class="cookbook-body">
        <CookbookFilters state={filters} />
        <CookbookRecipeGrid
          recipes={recipeList.recipes}
          loading={recipeList.loading}
          loadingMore={recipeList.loadingMore}
          hasMore={recipeList.hasMore}
          query={searchQuery}
          density={density}
          bulkMode={bulkSelection.bulkMode}
          selected={bulkSelection.selected}
          token={token}
          error={error}
          notice={notice}
          clearSearch={clearSearch}
          toggleSelected={bulkSelection.toggleSelected}
        />
      </div>

      <PdfExportModal
        isOpen={bulkSelection.showPdfModal}
        onClose={() => bulkSelection.setShowPdfModal(false)}
        recipes={bulkSelection.selectedRecipes}
        token={token}
        authedFetch={authedFetch}
      />
    </div>
  );
}
