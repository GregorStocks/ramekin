import {
  createSignal,
  createEffect,
  createMemo,
  Show,
  For,
  onMount,
  onCleanup,
} from "solid-js";
import { A, useNavigate, useSearchParams } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import { extractApiError } from "../utils/recipeFormHelpers";
import { usePageTitle } from "../utils/pageTitle";
import PhotoThumbnail from "../components/PhotoThumbnail";
import PdfExportModal from "../components/PdfExportModal";
import type { RecipeSummary, SortBy, Direction } from "ramekin-client";

interface NumericThreshold {
  op: "<" | ">";
  value: number;
}

interface FilterState {
  tags: string[];
  source: string;
  photos: "any" | "has" | "no";
  createdAfter: string;
  createdBefore: string;
  photoSize: NumericThreshold | null;
  photoDim: NumericThreshold | null;
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

function parseNumericThreshold(expr: string): NumericThreshold | null {
  if (expr.startsWith("<")) {
    const v = parseInt(expr.slice(1), 10);
    if (!isNaN(v)) return { op: "<", value: v };
  } else if (expr.startsWith(">")) {
    const v = parseInt(expr.slice(1), 10);
    if (!isNaN(v)) return { op: ">", value: v };
  }
  return null;
}

function buildThresholdFromInput(
  valueStr: string,
  op: "<" | ">",
): NumericThreshold | null {
  const trimmed = valueStr.trim();
  if (!trimmed) return null;
  const value = parseInt(trimmed, 10);
  if (isNaN(value)) return null;
  return { op, value };
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
    photoSize: null,
    photoDim: null,
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
    } else if (token.startsWith("photo_size:")) {
      filters.photoSize = parseNumericThreshold(token.slice(11));
    } else if (token.startsWith("photo_dim:")) {
      filters.photoDim = parseNumericThreshold(token.slice(10));
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

  for (const term of textTerms) {
    if (term.includes(" ")) {
      parts.push(`"${term}"`);
    } else {
      parts.push(term);
    }
  }

  for (const tag of filters.tags) {
    if (tag.includes(" ")) {
      parts.push(`tag:"${tag}"`);
    } else {
      parts.push(`tag:${tag}`);
    }
  }

  if (filters.source) {
    if (filters.source.includes(" ")) {
      parts.push(`source:"${filters.source}"`);
    } else {
      parts.push(`source:${filters.source}`);
    }
  }

  if (filters.photos === "has") {
    parts.push("has:photos");
  } else if (filters.photos === "no") {
    parts.push("no:photos");
  }

  if (filters.photoSize) {
    parts.push(`photo_size:${filters.photoSize.op}${filters.photoSize.value}`);
  }
  if (filters.photoDim) {
    parts.push(`photo_dim:${filters.photoDim.op}${filters.photoDim.value}`);
  }

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

type Density = "card" | "compact" | "list";
const DENSITY_KEY = "cookbookDensity";

function loadDensity(): Density {
  const v = localStorage.getItem(DENSITY_KEY);
  return v === "compact" || v === "list" ? v : "card";
}

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
  usePageTitle(() => "Cookbook");
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

  const getQueryParam = (param: string | string[] | undefined): string => {
    if (Array.isArray(param)) return param[0] || "";
    return param || "";
  };

  const [searchInput, setSearchInput] = createSignal(
    getQueryParam(searchParams.q),
  );

  // Mobile filter sheet visibility (desktop sidebar is always shown via CSS).
  const [mobileFiltersOpen, setMobileFiltersOpen] = createSignal(false);

  // Local input state for typed filter fields. Hydrates from URL whenever
  // the URL query changes; flushed back to the URL via patchFilters().
  const [sourceInput, setSourceInput] = createSignal("");
  const [photoSizeOp, setPhotoSizeOp] = createSignal<"<" | ">">("<");
  const [photoSizeInput, setPhotoSizeInput] = createSignal("");
  const [photoDimOp, setPhotoDimOp] = createSignal<"<" | ">">("<");
  const [photoDimInput, setPhotoDimInput] = createSignal("");

  // Bulk mode state
  const [bulkMode, setBulkMode] = createSignal(false);
  const [density, setDensity] = createSignal<Density>(loadDensity());
  createEffect(() => {
    localStorage.setItem(DENSITY_KEY, density());
  });
  const [selected, setSelected] = createSignal<Set<string>>(new Set());
  const [selectAllStatus, setSelectAllStatus] = createSignal<string | null>(
    null,
  );
  // Full list of matching recipes (loaded for bulk ops like PDF export).
  const [bulkRecipes, setBulkRecipes] = createSignal<RecipeSummary[]>([]);
  const [showPdfModal, setShowPdfModal] = createSignal(false);
  const [rescrapeProgress, setRescrapeProgress] = createSignal<{
    done: number;
    total: number;
  } | null>(null);
  const [normalizeTitleProgress, setNormalizeTitleProgress] = createSignal<{
    done: number;
    total: number;
  } | null>(null);
  const [descriptionProgress, setDescriptionProgress] = createSignal<{
    done: number;
    total: number;
  } | null>(null);

  const PAGE_SIZE = 20;

  const searchQuery = () => getQueryParam(searchParams.q);

  // URL is the single source of truth for filter state. Memoized so the ~45
  // tag chips share a single parse per URL change rather than reparsing on
  // every chip's classList read.
  const parsedQuery = createMemo(() => parseQueryToFilters(searchQuery()));
  const currentFilters = (): FilterState => parsedQuery().filters;
  const currentTextTerms = (): string[] => parsedQuery().textTerms;

  // Tags sorted alphabetically (case-insensitive) for stable chip ordering.
  const sortedTags = () =>
    [...availableTags()].sort((a, b) =>
      a.localeCompare(b, undefined, { sensitivity: "base" }),
    );

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

  const updateSearchQuery = (value: string) => {
    setSearchInput(value);
    const q = value.trim();
    if (q !== searchQuery()) {
      setSearchParams({ q: q || undefined });
    }
  };

  const handleSearch = (e?: Event) => {
    e?.preventDefault();
    updateSearchQuery(searchInput());
  };

  // Reload when search query or sort changes in URL
  createEffect(() => {
    const q = searchQuery();
    sortOption();
    setSearchInput(q);
    setOffset(0);
    setRecipes([]);
    // Leaving bulk mode on filter change would surprise the user; clear
    // selection, and bump bulkRequestId so any in-flight fetchAllMatching
    // sees the change and discards its results instead of writing stale
    // IDs back into bulkRecipes/selected.
    setSelected(new Set<string>());
    setBulkRecipes([]);
    setBulkRequestId((id) => id + 1);
    loadRecipes(false, 0);
  });

  // Hydrate local input fields whenever filter state changes in the URL.
  // Safe to overwrite because patchFilters() always commits pending local
  // edits before writing the URL, so the URL is the canonical truth.
  createEffect(() => {
    const f = currentFilters();
    setSourceInput(f.source);
    if (f.photoSize) {
      setPhotoSizeOp(f.photoSize.op);
      setPhotoSizeInput(String(f.photoSize.value));
    } else {
      setPhotoSizeInput("");
    }
    if (f.photoDim) {
      setPhotoDimOp(f.photoDim.op);
      setPhotoDimInput(String(f.photoDim.value));
    } else {
      setPhotoDimInput("");
    }
  });

  // Lock body scroll + listen for Escape while the mobile filter sheet is open.
  createEffect(() => {
    if (!mobileFiltersOpen()) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setMobileFiltersOpen(false);
    };
    document.addEventListener("keydown", onKey);
    document.body.style.overflow = "hidden";
    onCleanup(() => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = "";
    });
  });

  const handleScroll = () => {
    const scrollHeight = document.documentElement.scrollHeight;
    const scrollTop = document.documentElement.scrollTop;
    const clientHeight = document.documentElement.clientHeight;

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

  const activeFilterCount = () => {
    const f = currentFilters();
    let count = 0;
    if (f.tags.length > 0) count += f.tags.length;
    if (f.source) count++;
    if (f.photos !== "any") count++;
    if (f.createdAfter || f.createdBefore) count++;
    if (f.photoSize) count++;
    if (f.photoDim) count++;
    return count;
  };

  // Merge a partial filter change into the URL query, preserving free-text
  // search terms. Always commits any pending local input edits (source,
  // photo size/dim) before applying the patch so a chip click never silently
  // discards a half-typed text input.
  const patchFilters = (patch: Partial<FilterState>) => {
    const base = currentFilters();
    const next: FilterState = {
      ...base,
      source: sourceInput().trim(),
      photoSize: buildThresholdFromInput(photoSizeInput(), photoSizeOp()),
      photoDim: buildThresholdFromInput(photoDimInput(), photoDimOp()),
      ...patch,
    };
    const newQuery = buildQueryFromFilters(currentTextTerms(), next);
    setSearchInput(newQuery);
    setSearchParams({ q: newQuery || undefined });
  };

  const clearFilters = () => {
    // Empty the local inputs first so patchFilters' implicit flush rebuilds
    // the URL with cleared source/threshold values; the explicit patch
    // handles the URL-derived facets (tags, photos, dates).
    setSourceInput("");
    setPhotoSizeInput("");
    setPhotoDimInput("");
    patchFilters({
      tags: [],
      photos: "any",
      createdAfter: "",
      createdBefore: "",
    });
  };

  const toggleTag = (tag: string) => {
    const tags = currentFilters().tags;
    patchFilters({
      tags: tags.includes(tag) ? tags.filter((t) => t !== tag) : [...tags, tag],
    });
  };

  const flushLocalInputs = () => patchFilters({});

  const handleOpChange = (
    setOp: (op: "<" | ">") => void,
    input: () => string,
    op: "<" | ">",
  ) => {
    setOp(op);
    if (input().trim()) flushLocalInputs();
  };

  const handleFlushKeyDown = (
    e: KeyboardEvent & { currentTarget: HTMLInputElement },
  ) => {
    if (e.key === "Enter") {
      e.preventDefault();
      flushLocalInputs();
      e.currentTarget.blur();
    }
  };

  const renderThreshold = (
    label: string,
    placeholder: string,
    op: () => "<" | ">",
    setOp: (op: "<" | ">") => void,
    input: () => string,
    setInput: (v: string) => void,
  ) => (
    <div class="cookbook-filter-section">
      <div class="cookbook-filter-label">{label}</div>
      <div class="cookbook-filter-threshold-row">
        <select
          class="cookbook-filter-input cookbook-filter-input-op"
          value={op()}
          onChange={(e) =>
            handleOpChange(setOp, input, e.currentTarget.value as "<" | ">")
          }
        >
          <option value="<">&lt;</option>
          <option value=">">&gt;</option>
        </select>
        <input
          type="number"
          min="0"
          step="1"
          class="cookbook-filter-input"
          placeholder={placeholder}
          value={input()}
          onInput={(e) => setInput(e.currentTarget.value)}
          onBlur={flushLocalInputs}
          onKeyDown={handleFlushKeyDown}
        />
      </div>
    </div>
  );

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

  // --- Bulk mode helpers ---

  const toggleBulkMode = () => {
    if (bulkMode()) {
      setBulkMode(false);
      setSelected(new Set<string>());
      setBulkRecipes([]);
      // Bump the request ID so any in-flight selectAll discards its
      // results instead of repopulating selected/bulkRecipes after the
      // user has already exited bulk mode.
      setBulkRequestId((id) => id + 1);
    } else {
      setBulkMode(true);
    }
  };

  const toggleSelected = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  // Monotonically-increasing request ID used to discard stale select-all
  // responses. If the user changes filters/sort while fetchAllMatching is
  // in flight, the createEffect below bumps bulkRequestId so the older
  // request's results are ignored instead of being written back to state
  // (where they'd silently target the wrong recipes).
  const [bulkRequestId, setBulkRequestId] = createSignal(0);

  // Fetch *every* recipe matching the current filter (across all pages).
  // Returns the full list, or null if the request was superseded by a newer
  // one (filter changed mid-flight). Also caches the result on bulkRecipes()
  // when still current, so later actions don't need to refetch. We always
  // paginate with a deterministic sort (updated_at desc) regardless of the
  // user's visible sort — otherwise random order gives a different shuffle
  // per page, so offset pagination would duplicate and skip rows and the
  // resulting Set would silently miss matching recipes.
  const fetchAllMatching = async (): Promise<RecipeSummary[] | null> => {
    const cached = bulkRecipes();
    if (cached.length > 0 && cached.length === total()) {
      return cached;
    }
    const myRequestId = bulkRequestId() + 1;
    setBulkRequestId(myRequestId);
    const q = searchQuery();
    const api = getRecipesApi();
    const all: RecipeSummary[] = [];
    const pageSize = 200;
    let offset = 0;
    while (true) {
      const resp = await api.listRecipes({
        limit: pageSize,
        offset,
        q: q || undefined,
        sortBy: "updated_at",
        sortDir: "desc",
      });
      // If a newer request was started (filter changed), bail out without
      // touching bulk state.
      if (bulkRequestId() !== myRequestId) {
        return null;
      }
      all.push(...resp.recipes);
      offset += resp.recipes.length;
      if (resp.recipes.length === 0 || offset >= resp.pagination.total) {
        break;
      }
    }
    if (bulkRequestId() !== myRequestId) {
      return null;
    }
    setBulkRecipes(all);
    return all;
  };

  const selectAll = async () => {
    setSelectAllStatus("Loading…");
    try {
      const all = await fetchAllMatching();
      if (all === null) {
        // Filter changed while we were loading — selection was already
        // cleared by the createEffect and bulkRecipes was not mutated.
        setSelectAllStatus(null);
        return;
      }
      setSelected(new Set(all.map((r) => r.id)));
      setSelectAllStatus(null);
    } catch (e) {
      setSelectAllStatus(null);
      setError("Failed to load all recipes");
    }
  };

  const clearSelection = () => setSelected(new Set<string>());

  const selectedRecipes = (): RecipeSummary[] => {
    const ids = selected();
    if (ids.size === 0) return [];
    // Prefer bulkRecipes (full set) if populated, else fall back to visible list
    const source = bulkRecipes().length > 0 ? bulkRecipes() : recipes();
    return source.filter((r) => ids.has(r.id));
  };

  const openPdfExport = async () => {
    // If user selected items that aren't all in the visible page, we still need
    // their details to generate the PDF. Make sure we have the full set.
    if (bulkRecipes().length === 0 && selected().size > recipes().length) {
      try {
        await fetchAllMatching();
      } catch (e) {
        setError("Failed to load recipes for PDF");
        return;
      }
    }
    setShowPdfModal(true);
  };

  const bulkNormalizeTitle = async () => {
    const ids = Array.from(selected());
    if (ids.length === 0) return;
    const confirmMsg =
      ids.length === 1
        ? "Normalize (de-clickbait) the title of this recipe?"
        : `Normalize (de-clickbait) titles for ${ids.length} recipes? Each one calls the LLM (cached results are free).`;
    if (!window.confirm(confirmMsg)) return;

    setNormalizeTitleProgress({ done: 0, total: ids.length });
    const api = getRecipesApi();
    let done = 0;
    let changed = 0;
    const errors: string[] = [];
    for (const id of ids) {
      try {
        const res = await api.normalizeTitle({ id });
        if (res.changed) changed += 1;
      } catch (e) {
        const msg = await extractApiError(e, "normalize failed");
        errors.push(`${id.slice(0, 8)}: ${msg}`);
      }
      done += 1;
      setNormalizeTitleProgress({ done, total: ids.length });
    }
    setNormalizeTitleProgress(null);
    await loadRecipes();
    if (errors.length > 0) {
      setError(
        `${ids.length - errors.length}/${ids.length} normalized (${changed} changed). Errors: ${errors.slice(0, 3).join("; ")}${errors.length > 3 ? "…" : ""}`,
      );
    } else {
      setError(
        `Normalized ${ids.length} recipes (${changed} changed, ${ids.length - changed} unchanged).`,
      );
    }
  };

  const bulkGenerateDescription = async () => {
    const ids = Array.from(selected());
    if (ids.length === 0) return;
    const confirmMsg =
      ids.length === 1
        ? "Generate a description for this recipe?"
        : `Generate descriptions for ${ids.length} recipes? Each one calls the LLM (cached results are free).`;
    if (!window.confirm(confirmMsg)) return;

    setDescriptionProgress({ done: 0, total: ids.length });
    const api = getRecipesApi();
    let done = 0;
    let changed = 0;
    const errors: string[] = [];
    for (const id of ids) {
      try {
        const res = await api.generateDescription({ id });
        if (res.changed) changed += 1;
      } catch (e) {
        const msg = await extractApiError(e, "description failed");
        errors.push(`${id.slice(0, 8)}: ${msg}`);
      }
      done += 1;
      setDescriptionProgress({ done, total: ids.length });
    }
    setDescriptionProgress(null);
    await loadRecipes();
    if (errors.length > 0) {
      setError(
        `${ids.length - errors.length}/${ids.length} described (${changed} changed). Errors: ${errors.slice(0, 3).join("; ")}${errors.length > 3 ? "…" : ""}`,
      );
    } else {
      setError(
        `Generated descriptions for ${ids.length} recipes (${changed} changed, ${ids.length - changed} unchanged).`,
      );
    }
  };

  const bulkRescrapePhoto = async () => {
    const ids = Array.from(selected());
    if (ids.length === 0) return;
    const confirmMsg =
      ids.length === 1
        ? "Queue a photo rescrape for this recipe?"
        : `Queue photo rescrapes for ${ids.length} recipes? This will issue one job per recipe.`;
    if (!window.confirm(confirmMsg)) return;

    setRescrapeProgress({ done: 0, total: ids.length });
    const api = getRecipesApi();
    let done = 0;
    const errors: string[] = [];
    for (const id of ids) {
      try {
        await api.rescrapePhoto({ id });
      } catch (e) {
        const msg = await extractApiError(e, "rescrape failed");
        errors.push(`${id.slice(0, 8)}: ${msg}`);
      }
      done += 1;
      setRescrapeProgress({ done, total: ids.length });
    }
    setRescrapeProgress(null);
    if (errors.length > 0) {
      setError(
        `${ids.length - errors.length}/${ids.length} jobs queued. Errors: ${errors.slice(0, 3).join("; ")}${errors.length > 3 ? "…" : ""}`,
      );
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

      <div class="cookbook-utility-bar">
        <form class="search-bar" onSubmit={handleSearch}>
          <input
            type="text"
            class="search-input"
            placeholder="Search recipes..."
            value={searchInput()}
            onInput={(e) => updateSearchQuery(e.currentTarget.value)}
          />
          <Show when={searchInput()}>
            <button type="button" class="search-clear" onClick={clearSearch}>
              &times;
            </button>
          </Show>
        </form>
        <button
          type="button"
          class="filter-button cookbook-mobile-filters-toggle"
          onClick={() => setMobileFiltersOpen(true)}
          classList={{ active: activeFilterCount() > 0 }}
          aria-label="Open filters"
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
        <button
          type="button"
          class="filter-button"
          onClick={toggleBulkMode}
          classList={{ active: bulkMode() }}
        >
          {bulkMode() ? "Done" : "Select"}
        </button>
        <div class="density-toggle" role="group" aria-label="Recipe density">
          <For each={["card", "compact", "list"] as const}>
            {(mode) => (
              <button
                type="button"
                class="density-toggle-button"
                classList={{ active: density() === mode }}
                aria-pressed={density() === mode}
                onClick={() => setDensity(mode)}
              >
                {mode === "card"
                  ? "Cards"
                  : mode === "compact"
                    ? "Compact"
                    : "List"}
              </button>
            )}
          </For>
        </div>
      </div>

      <Show when={bulkMode()}>
        <div class="bulk-toolbar">
          <span class="bulk-count">
            {selected().size} selected
            <Show when={selectAllStatus()}>
              {" "}
              · <em>{selectAllStatus()}</em>
            </Show>
          </span>
          <button
            type="button"
            class="btn btn-small"
            onClick={selectAll}
            disabled={total() === 0 || selectAllStatus() !== null}
          >
            Select all ({total()})
          </button>
          <button
            type="button"
            class="btn btn-small"
            onClick={clearSelection}
            disabled={selected().size === 0}
          >
            Clear
          </button>
          <button
            type="button"
            class="btn btn-small btn-primary"
            onClick={openPdfExport}
            disabled={selected().size === 0}
          >
            Export to PDF
          </button>
          <button
            type="button"
            class="btn btn-small"
            onClick={bulkRescrapePhoto}
            disabled={selected().size === 0 || rescrapeProgress() !== null}
          >
            <Show when={rescrapeProgress()} fallback={<>Rescrape photo</>}>
              Rescraping {rescrapeProgress()!.done}/{rescrapeProgress()!.total}…
            </Show>
          </button>
          <button
            type="button"
            class="btn btn-small"
            onClick={bulkNormalizeTitle}
            disabled={
              selected().size === 0 || normalizeTitleProgress() !== null
            }
          >
            <Show when={normalizeTitleProgress()} fallback={<>Auto-rename</>}>
              Renaming {normalizeTitleProgress()!.done}/
              {normalizeTitleProgress()!.total}…
            </Show>
          </button>
          <button
            type="button"
            class="btn btn-small"
            onClick={bulkGenerateDescription}
            disabled={selected().size === 0 || descriptionProgress() !== null}
          >
            <Show
              when={descriptionProgress()}
              fallback={<>Generate descriptions</>}
            >
              Describing {descriptionProgress()!.done}/
              {descriptionProgress()!.total}…
            </Show>
          </button>
        </div>
      </Show>

      <div class="cookbook-body">
        <aside
          class="cookbook-sidebar"
          classList={{ "cookbook-sidebar-open": mobileFiltersOpen() }}
          aria-label="Filters"
        >
          <div class="cookbook-sidebar-header">
            <h3 class="cookbook-sidebar-title">Filters</h3>
            <button
              type="button"
              class="cookbook-sidebar-close"
              onClick={() => setMobileFiltersOpen(false)}
              aria-label="Close filters"
            >
              ✕
            </button>
          </div>

          <div class="cookbook-filter-section">
            <div class="cookbook-filter-label">Tags</div>
            <Show
              when={sortedTags().length > 0}
              fallback={<span class="cookbook-filter-empty">No tags yet</span>}
            >
              <div class="cookbook-filter-chips">
                <For each={sortedTags()}>
                  {(tag) => (
                    <button
                      type="button"
                      class="filter-chip"
                      classList={{
                        "filter-chip-active":
                          currentFilters().tags.includes(tag),
                      }}
                      aria-pressed={currentFilters().tags.includes(tag)}
                      onClick={() => toggleTag(tag)}
                    >
                      {tag}
                    </button>
                  )}
                </For>
              </div>
            </Show>
          </div>

          <div class="cookbook-filter-section">
            <label class="cookbook-filter-label" for="cookbook-filter-source">
              Source
            </label>
            <input
              id="cookbook-filter-source"
              type="text"
              class="cookbook-filter-input"
              placeholder="e.g. NYTimes"
              value={sourceInput()}
              onInput={(e) => setSourceInput(e.currentTarget.value)}
              onBlur={flushLocalInputs}
              onKeyDown={handleFlushKeyDown}
            />
          </div>

          <div class="cookbook-filter-section">
            <div class="cookbook-filter-label">Photos</div>
            <div class="cookbook-filter-radio-group">
              <label class="cookbook-filter-radio">
                <input
                  type="radio"
                  name="photos"
                  checked={currentFilters().photos === "any"}
                  onChange={() => patchFilters({ photos: "any" })}
                />
                Any
              </label>
              <label class="cookbook-filter-radio">
                <input
                  type="radio"
                  name="photos"
                  checked={currentFilters().photos === "has"}
                  onChange={() => patchFilters({ photos: "has" })}
                />
                Has photos
              </label>
              <label class="cookbook-filter-radio">
                <input
                  type="radio"
                  name="photos"
                  checked={currentFilters().photos === "no"}
                  onChange={() => patchFilters({ photos: "no" })}
                />
                No photos
              </label>
            </div>
          </div>

          {renderThreshold(
            "Photo file size (bytes)",
            "e.g. 100000",
            photoSizeOp,
            setPhotoSizeOp,
            photoSizeInput,
            setPhotoSizeInput,
          )}

          {renderThreshold(
            "Photo dimensions (min side, px)",
            "e.g. 600",
            photoDimOp,
            setPhotoDimOp,
            photoDimInput,
            setPhotoDimInput,
          )}

          <div class="cookbook-filter-section">
            <div class="cookbook-filter-label">Created</div>
            <div class="cookbook-filter-date-range">
              <input
                type="date"
                class="cookbook-filter-input"
                value={currentFilters().createdAfter}
                onInput={(e) =>
                  patchFilters({ createdAfter: e.currentTarget.value })
                }
              />
              <span>to</span>
              <input
                type="date"
                class="cookbook-filter-input"
                value={currentFilters().createdBefore}
                onInput={(e) =>
                  patchFilters({ createdBefore: e.currentTarget.value })
                }
              />
            </div>
          </div>

          <div class="cookbook-filter-actions">
            <button
              type="button"
              class="btn btn-small"
              onClick={clearFilters}
              disabled={activeFilterCount() === 0}
            >
              Clear all filters
            </button>
          </div>
        </aside>

        <Show when={mobileFiltersOpen()}>
          <div
            class="cookbook-sidebar-overlay"
            aria-hidden="true"
            onClick={() => setMobileFiltersOpen(false)}
          />
        </Show>

        <div class="cookbook-content">
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
            <div class="recipe-grid" data-density={density()}>
              <For each={recipes()}>
                {(recipe) => {
                  const card = (
                    <>
                      <Show
                        when={recipe.thumbnailPhotoId}
                        fallback={<div class="recipe-card-placeholder">🍽️</div>}
                      >
                        <PhotoThumbnail
                          photoId={recipe.thumbnailPhotoId!}
                          token={token()!}
                          alt={recipe.title}
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
                    </>
                  );

                  return (
                    <Show
                      when={bulkMode()}
                      fallback={
                        <A href={`/recipes/${recipe.id}`} class="recipe-card">
                          {card}
                        </A>
                      }
                    >
                      <div
                        class="recipe-card recipe-card-selectable"
                        classList={{ selected: selected().has(recipe.id) }}
                        onClick={() => toggleSelected(recipe.id)}
                      >
                        <input
                          type="checkbox"
                          class="recipe-card-checkbox"
                          checked={selected().has(recipe.id)}
                          onClick={(e) => e.stopPropagation()}
                          onChange={() => toggleSelected(recipe.id)}
                        />
                        {card}
                      </div>
                    </Show>
                  );
                }}
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
                style={{
                  "text-align": "center",
                  padding: "2rem",
                  color: "#666",
                }}
              >
                No more recipes
              </p>
            </Show>
          </Show>
        </div>
      </div>

      <PdfExportModal
        isOpen={showPdfModal}
        onClose={() => setShowPdfModal(false)}
        recipes={selectedRecipes}
        token={token}
      />
    </div>
  );
}
