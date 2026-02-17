import {
  createSignal,
  createEffect,
  Show,
  For,
  onMount,
  onCleanup,
} from "solid-js";
import { A, useNavigate, useSearchParams } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import { extractApiError } from "../utils/recipeFormHelpers";
import PhotoThumbnail from "../components/PhotoThumbnail";
import type { RecipeSummary, SortBy, Direction } from "ramekin-client";

interface FilterState {
  tags: string[];
  source: string;
  photos: "any" | "has" | "no";
  createdAfter: string;
  createdBefore: string;
}

type SortOption =
  | "newest"
  | "oldest"
  | "rating"
  | "title"
  | "created"
  | "random";

function getSortParams(sort: SortOption): {
  sortBy: SortBy;
  sortDir?: Direction;
} {
  switch (sort) {
    case "oldest":
      return { sortBy: "updated_at", sortDir: "asc" };
    case "rating":
      return { sortBy: "rating", sortDir: "desc" };
    case "title":
      return { sortBy: "title", sortDir: "asc" };
    case "created":
      return { sortBy: "created_at", sortDir: "desc" };
    case "random":
      return { sortBy: "random" };
    case "newest":
    default:
      return { sortBy: "updated_at", sortDir: "desc" };
  }
}

function parseQueryToFilters(query: string): {
  textTerms: string[];
  filters: FilterState;
} {
  const filters: FilterState = {
    tags: [],
    source: "",
    photos: "any",
    createdAfter: "",
    createdBefore: "",
  };
  const textTerms: string[] = [];

  // Simple tokenizer - split on whitespace, but respect quotes
  const tokens: string[] = [];
  let current = "";
  let inQuotes = false;
  for (const c of query) {
    if (c === '"') {
      inQuotes = !inQuotes;
    } else if ((c === " " || c === "\t") && !inQuotes) {
      if (current) {
        tokens.push(current);
        current = "";
      }
    } else {
      current += c;
    }
  }
  if (current) tokens.push(current);

  for (const token of tokens) {
    if (token.startsWith("tag:")) {
      const tag = token.slice(4);
      if (tag) filters.tags.push(tag);
    } else if (token.startsWith("source:")) {
      const source = token.slice(7);
      if (source) filters.source = source;
    } else if (token === "has:photos" || token === "has:photo") {
      filters.photos = "has";
    } else if (token === "no:photos" || token === "no:photo") {
      filters.photos = "no";
    } else if (token.startsWith("created:")) {
      const expr = token.slice(8);
      if (expr.includes("..")) {
        const [start, end] = expr.split("..");
        if (start) filters.createdAfter = start;
        if (end) filters.createdBefore = end;
      } else if (expr.startsWith(">")) {
        filters.createdAfter = expr.slice(1);
      } else if (expr.startsWith("<")) {
        filters.createdBefore = expr.slice(1);
      } else {
        // Exact date - treat as range for same day
        filters.createdAfter = expr;
        filters.createdBefore = expr;
      }
    } else if (token) {
      textTerms.push(token);
    }
  }

  return { textTerms, filters };
}

function buildQueryFromFilters(
  textTerms: string[],
  filters: FilterState,
): string {
  const parts: string[] = [];

  // Add text terms (quote if contains spaces)
  for (const term of textTerms) {
    if (term.includes(" ")) {
      parts.push(`"${term}"`);
    } else {
      parts.push(term);
    }
  }

  // Add tags (quote if contains spaces)
  for (const tag of filters.tags) {
    if (tag.includes(" ")) {
      parts.push(`tag:"${tag}"`);
    } else {
      parts.push(`tag:${tag}`);
    }
  }

  // Add source (quote if contains spaces)
  if (filters.source) {
    if (filters.source.includes(" ")) {
      parts.push(`source:"${filters.source}"`);
    } else {
      parts.push(`source:${filters.source}`);
    }
  }

  // Add photos filter
  if (filters.photos === "has") {
    parts.push("has:photos");
  } else if (filters.photos === "no") {
    parts.push("no:photos");
  }

  // Add date filters
  if (filters.createdAfter && filters.createdBefore) {
    if (filters.createdAfter === filters.createdBefore) {
      parts.push(`created:${filters.createdAfter}`);
    } else {
      parts.push(`created:${filters.createdAfter}..${filters.createdBefore}`);
    }
  } else if (filters.createdAfter) {
    parts.push(`created:>${filters.createdAfter}`);
  } else if (filters.createdBefore) {
    parts.push(`created:<${filters.createdBefore}`);
  }

  return parts.join(" ");
}

const thumbnailSize = window.devicePixelRatio >= 2 ? 800 : 400;

function formatRelativeDate(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) {
    return "Updated today";
  } else if (diffDays === 1) {
    return "Updated yesterday";
  } else if (diffDays < 7) {
    return `Updated ${diffDays} days ago`;
  } else {
    return `Updated ${date.toLocaleDateString("en-US", { month: "short", day: "numeric" })}`;
  }
}

export default function CookbookPage() {
  const { getRecipesApi, tags: availableTags, token } = useAuth();
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();

  const [recipes, setRecipes] = createSignal<RecipeSummary[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [loadingMore, setLoadingMore] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [offset, setOffset] = createSignal(0);
  const [total, setTotal] = createSignal(0);
  const [hasMore, setHasMore] = createSignal(true);

  // Search input state (local, updates URL on submit/blur)
  const getQueryParam = (param: string | string[] | undefined): string => {
    if (Array.isArray(param)) return param[0] || "";
    return param || "";
  };

  const [searchInput, setSearchInput] = createSignal(
    getQueryParam(searchParams.q),
  );

  // Filter panel state
  const [showFilters, setShowFilters] = createSignal(false);
  const [filterTags, setFilterTags] = createSignal<string[]>([]);
  const [filterSource, setFilterSource] = createSignal("");
  const [filterPhotos, setFilterPhotos] = createSignal<"any" | "has" | "no">(
    "any",
  );
  const [filterCreatedAfter, setFilterCreatedAfter] = createSignal("");
  const [filterCreatedBefore, setFilterCreatedBefore] = createSignal("");

  // Track text terms from search (non-filter parts)
  const [textTerms, setTextTerms] = createSignal<string[]>([]);

  const PAGE_SIZE = 20;

  // Get current search query from URL
  const searchQuery = () => getQueryParam(searchParams.q);

  // Get current sort from URL (default to "newest")
  const sortOption = (): SortOption => {
    const sort = getQueryParam(searchParams.sort);
    if (
      sort === "oldest" ||
      sort === "rating" ||
      sort === "title" ||
      sort === "created" ||
      sort === "random"
    )
      return sort;
    return "newest";
  };

  const handleSortChange = (e: Event) => {
    const value = (e.target as HTMLSelectElement).value as SortOption;
    setSearchParams({ sort: value === "newest" ? undefined : value });
  };

  const loadRecipes = async (appendMode = false, currentOffset = 0) => {
    if (appendMode) {
      setLoadingMore(true);
    } else {
      setLoading(true);
    }
    setError(null);

    try {
      const q = searchQuery();
      const { sortBy, sortDir } = getSortParams(sortOption());
      const response = await getRecipesApi().listRecipes({
        limit: PAGE_SIZE,
        offset: currentOffset,
        q: q || undefined,
        sortBy,
        sortDir,
      });

      if (appendMode) {
        setRecipes([...recipes(), ...response.recipes]);
      } else {
        setRecipes(response.recipes);
      }

      setTotal(response.pagination.total);
      setOffset(currentOffset + response.recipes.length);
      setHasMore(
        currentOffset + response.recipes.length < response.pagination.total,
      );
    } catch (err) {
      setError("Failed to load recipes");
    } finally {
      setLoading(false);
      setLoadingMore(false);
    }
  };

  const loadMore = () => {
    if (!loadingMore() && hasMore()) {
      loadRecipes(true, offset());
    }
  };

  // Handle search submission
  const handleSearch = (e?: Event) => {
    e?.preventDefault();
    const q = searchInput().trim();
    if (q !== searchQuery()) {
      setSearchParams({ q: q || undefined });
    }
  };

  // Reload when search query or sort changes in URL
  createEffect(() => {
    const q = searchQuery();
    sortOption(); // Track sort changes
    // Sync input with URL
    setSearchInput(q);
    // Reset and reload
    setOffset(0);
    setRecipes([]);
    loadRecipes(false, 0);
  });

  // Scroll listener for infinite scroll
  const handleScroll = () => {
    const scrollHeight = document.documentElement.scrollHeight;
    const scrollTop = document.documentElement.scrollTop;
    const clientHeight = document.documentElement.clientHeight;

    // Load more when user is within 300px of bottom
    if (scrollHeight - scrollTop - clientHeight < 300) {
      loadMore();
    }
  };

  onMount(() => {
    window.addEventListener("scroll", handleScroll);
  });

  onCleanup(() => {
    window.removeEventListener("scroll", handleScroll);
  });

  const recipeCount = () => {
    const count = recipes().length;
    const totalCount = total();
    if (totalCount === 0) return "";
    if (count < totalCount) {
      return `(showing ${count} of ${totalCount} recipes)`;
    }
    if (count === 1) return "(1 recipe)";
    return `(${count} recipes)`;
  };

  const clearSearch = () => {
    setSearchInput("");
    setSearchParams({ q: undefined });
  };

  // Count active filters for button badge
  const activeFilterCount = () => {
    let count = 0;
    if (filterTags().length > 0) count += filterTags().length;
    if (filterSource()) count++;
    if (filterPhotos() !== "any") count++;
    if (filterCreatedAfter() || filterCreatedBefore()) count++;
    return count;
  };

  // Open filter panel and sync from current query
  const openFilters = () => {
    const { textTerms: terms, filters } = parseQueryToFilters(searchQuery());
    setTextTerms(terms);
    setFilterTags(filters.tags);
    setFilterSource(filters.source);
    setFilterPhotos(filters.photos);
    setFilterCreatedAfter(filters.createdAfter);
    setFilterCreatedBefore(filters.createdBefore);
    setShowFilters(true);
  };

  // Apply filters and close panel
  const applyFilters = () => {
    const newQuery = buildQueryFromFilters(textTerms(), {
      tags: filterTags(),
      source: filterSource(),
      photos: filterPhotos(),
      createdAfter: filterCreatedAfter(),
      createdBefore: filterCreatedBefore(),
    });
    setSearchInput(newQuery);
    setSearchParams({ q: newQuery || undefined });
    setShowFilters(false);
  };

  // Clear all filters
  const clearFilters = () => {
    setFilterTags([]);
    setFilterSource("");
    setFilterPhotos("any");
    setFilterCreatedAfter("");
    setFilterCreatedBefore("");
  };

  // Toggle a tag in the filter
  const toggleTag = (tag: string) => {
    const current = filterTags();
    if (current.includes(tag)) {
      setFilterTags(current.filter((t) => t !== tag));
    } else {
      setFilterTags([...current, tag]);
    }
  };

  // Navigate to a random recipe from current filter
  const goToRandomRecipe = async () => {
    try {
      const q = searchQuery();
      const response = await getRecipesApi().listRecipes({
        q: q || undefined,
        limit: 1,
        sortBy: "random",
      });
      if (response.recipes.length > 0) {
        navigate(
          `/recipes/${response.recipes[0].id}?randomQ=${encodeURIComponent(q || "")}`,
        );
      }
    } catch (err) {
      const message = await extractApiError(
        err,
        "Failed to load random recipe",
      );
      setError(message);
    }
  };

  return (
    <div class="cookbook-page">
      <div class="page-header">
        <h2>
          My Cookbook{" "}
          <Show when={!loading() && total() > 0}>
            <span class="recipe-count">{recipeCount()}</span>
          </Show>
        </h2>
      </div>

      <div class="search-container">
        <form class="search-bar" onSubmit={handleSearch}>
          <input
            type="text"
            class="search-input"
            placeholder="Search recipes..."
            value={searchInput()}
            onInput={(e) => setSearchInput(e.currentTarget.value)}
            onBlur={() => handleSearch()}
          />
          <Show when={searchInput()}>
            <button type="button" class="search-clear" onClick={clearSearch}>
              &times;
            </button>
          </Show>
        </form>
        <button
          type="button"
          class="filter-button"
          onClick={openFilters}
          classList={{ active: activeFilterCount() > 0 }}
        >
          Filters
          <Show when={activeFilterCount() > 0}>
            <span class="filter-badge">{activeFilterCount()}</span>
          </Show>
        </button>
        <select
          class="sort-select"
          value={sortOption()}
          onChange={handleSortChange}
        >
          <option value="newest">Newest first</option>
          <option value="oldest">Oldest first</option>
          <option value="rating">Highest rated</option>
          <option value="title">Title A–Z</option>
          <option value="created">Date added</option>
          <option value="random">Random order</option>
        </select>
        <button
          type="button"
          class="filter-button"
          onClick={goToRandomRecipe}
          disabled={total() === 0}
        >
          Random
        </button>
      </div>

      <Show when={showFilters()}>
        <div class="filter-panel">
          <div class="filter-section">
            <label class="filter-label">Tags</label>
            <div class="filter-tags">
              <Show
                when={availableTags().length > 0}
                fallback={<span class="filter-empty">No tags yet</span>}
              >
                <For each={availableTags()}>
                  {(tag) => (
                    <label class="filter-tag-option">
                      <input
                        type="checkbox"
                        checked={filterTags().includes(tag)}
                        onChange={() => toggleTag(tag)}
                      />
                      {tag}
                    </label>
                  )}
                </For>
              </Show>
            </div>
          </div>

          <div class="filter-section">
            <label class="filter-label">Source</label>
            <input
              type="text"
              class="filter-input"
              placeholder="e.g. NYTimes"
              value={filterSource()}
              onInput={(e) => setFilterSource(e.currentTarget.value)}
            />
          </div>

          <div class="filter-section">
            <label class="filter-label">Photos</label>
            <div class="filter-radio-group">
              <label class="filter-radio">
                <input
                  type="radio"
                  name="photos"
                  checked={filterPhotos() === "any"}
                  onChange={() => setFilterPhotos("any")}
                />
                Any
              </label>
              <label class="filter-radio">
                <input
                  type="radio"
                  name="photos"
                  checked={filterPhotos() === "has"}
                  onChange={() => setFilterPhotos("has")}
                />
                Has photos
              </label>
              <label class="filter-radio">
                <input
                  type="radio"
                  name="photos"
                  checked={filterPhotos() === "no"}
                  onChange={() => setFilterPhotos("no")}
                />
                No photos
              </label>
            </div>
          </div>

          <div class="filter-section">
            <label class="filter-label">Created</label>
            <div class="filter-date-range">
              <input
                type="date"
                class="filter-input"
                value={filterCreatedAfter()}
                onInput={(e) => setFilterCreatedAfter(e.currentTarget.value)}
              />
              <span>to</span>
              <input
                type="date"
                class="filter-input"
                value={filterCreatedBefore()}
                onInput={(e) => setFilterCreatedBefore(e.currentTarget.value)}
              />
            </div>
          </div>

          <div class="filter-actions">
            <button type="button" class="btn btn-small" onClick={clearFilters}>
              Clear
            </button>
            <button
              type="button"
              class="btn btn-small"
              onClick={() => setShowFilters(false)}
            >
              Cancel
            </button>
            <button
              type="button"
              class="btn btn-small btn-primary"
              onClick={applyFilters}
            >
              Apply
            </button>
          </div>
        </div>
      </Show>

      <Show when={loading()}>
        <p class="loading">Loading recipes...</p>
      </Show>

      <Show when={error()}>
        <p class="error">{error()}</p>
      </Show>

      <Show when={!loading() && recipes().length === 0 && !searchQuery()}>
        <div class="empty-state">
          <div class="empty-state-icon">📖</div>
          <h3>Your cookbook is empty</h3>
          <p>Start building your collection by adding your first recipe.</p>
          <A href="/recipes/new" class="btn btn-primary">
            + Add Your First Recipe
          </A>
        </div>
      </Show>

      <Show when={!loading() && recipes().length === 0 && searchQuery()}>
        <div class="empty-state">
          <div class="empty-state-icon">🔍</div>
          <h3>No recipes found</h3>
          <p>Try a different search term or clear the search.</p>
          <button class="btn btn-primary" onClick={clearSearch}>
            Clear Search
          </button>
        </div>
      </Show>

      <Show when={!loading() && recipes().length > 0}>
        <div class="recipe-grid">
          <For each={recipes()}>
            {(recipe) => (
              <A href={`/recipes/${recipe.id}`} class="recipe-card">
                <Show
                  when={recipe.thumbnailPhotoId}
                  fallback={<div class="recipe-card-placeholder">🍽️</div>}
                >
                  <PhotoThumbnail
                    photoId={recipe.thumbnailPhotoId!}
                    token={token()!}
                    alt=""
                    thumbnailSize={thumbnailSize}
                    class="recipe-card-thumbnail"
                  />
                </Show>
                <div class="recipe-card-content">
                  <h3>{recipe.title}</h3>
                  <Show when={recipe.description}>
                    <p class="recipe-description">{recipe.description}</p>
                  </Show>
                  <Show when={recipe.tags && recipe.tags.length > 0}>
                    <div class="recipe-tags">
                      <For each={recipe.tags!.slice(0, 3)}>
                        {(tag) => <span class="tag">{tag}</span>}
                      </For>
                      <Show when={recipe.tags!.length > 3}>
                        <span class="tag tag-more">
                          +{recipe.tags!.length - 3}
                        </span>
                      </Show>
                    </div>
                  </Show>
                  <p class="recipe-date">
                    {formatRelativeDate(recipe.updatedAt)}
                  </p>
                </div>
              </A>
            )}
          </For>
        </div>

        <Show when={loadingMore()}>
          <p
            class="loading"
            style={{ "text-align": "center", padding: "2rem" }}
          >
            Loading more recipes...
          </p>
        </Show>

        <Show when={!loadingMore() && !hasMore()}>
          <p
            class="loading"
            style={{ "text-align": "center", padding: "2rem", color: "#666" }}
          >
            No more recipes
          </p>
        </Show>
      </Show>
    </div>
  );
}
