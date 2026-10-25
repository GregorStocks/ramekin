import {
  buildQueryFromFilters,
  parseQueryToFilters,
  parseSortOption,
  type SortOption,
} from "./query";

const SORT_KEY = "cookbookSort";
const FILTERS_KEY = "cookbookFilters";

export type CookbookUrlParams = {
  q?: string;
  sort?: string;
};

/**
 * Remembers the last sort and filters so a fresh visit to the cookbook picks up
 * where it left off, like the iOS recipe list. Search text is not remembered.
 */
export function saveCookbookView(
  storage: Pick<Storage, "setItem">,
  sort: SortOption,
  query: string,
): void {
  storage.setItem(SORT_KEY, sort);
  storage.setItem(
    FILTERS_KEY,
    buildQueryFromFilters([], parseQueryToFilters(query).filters),
  );
}

/**
 * The URL params to restore for whichever of sort and query the URL leaves
 * out, or null when there is nothing to restore. Params in the URL win, so a
 * link like `/?q=tag:dinner` still shows exactly that tag.
 */
export function restoreCookbookView(
  storage: Pick<Storage, "getItem">,
  params: CookbookUrlParams,
): CookbookUrlParams | null {
  const restored: CookbookUrlParams = {};
  if (params.sort === undefined) {
    const sort = parseSortOption(storage.getItem(SORT_KEY) ?? "");
    if (sort !== "newest") restored.sort = sort;
  }
  if (params.q === undefined) {
    const filters = storage.getItem(FILTERS_KEY);
    if (filters) restored.q = filters;
  }
  return Object.keys(restored).length > 0 ? restored : null;
}
