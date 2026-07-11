import type { Direction, SortBy } from "ramekin-client";

export interface NumericThreshold {
  op: "<" | ">";
  value: number;
}

export interface FilterState {
  tags: string[];
  source: string;
  photos: "any" | "has" | "no";
  createdAfter: string;
  createdBefore: string;
  photoSize: NumericThreshold | null;
  photoDim: NumericThreshold | null;
}

export type SortOption =
  | "best"
  | "newest"
  | "oldest"
  | "rating"
  | "title"
  | "created"
  | "random";

export function getQueryParam(param: string | string[] | undefined): string {
  if (Array.isArray(param)) return param[0] || "";
  return param || "";
}

export function getSortParams(sort: SortOption): {
  sortBy?: SortBy;
  sortDir?: Direction;
} {
  switch (sort) {
    case "newest":
      return { sortBy: "updated_at", sortDir: "desc" };
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
    case "best":
    default:
      return {};
  }
}

export function parseSortOption(sort: string): SortOption {
  if (
    sort === "newest" ||
    sort === "oldest" ||
    sort === "rating" ||
    sort === "title" ||
    sort === "created" ||
    sort === "random"
  ) {
    return sort;
  }
  return "newest";
}

export function parseNumericThreshold(expr: string): NumericThreshold | null {
  if (expr.startsWith("<")) {
    const value = parseInt(expr.slice(1), 10);
    if (!Number.isNaN(value)) return { op: "<", value };
  } else if (expr.startsWith(">")) {
    const value = parseInt(expr.slice(1), 10);
    if (!Number.isNaN(value)) return { op: ">", value };
  }
  return null;
}

export function buildThresholdFromInput(
  valueStr: string,
  op: "<" | ">",
): NumericThreshold | null {
  const trimmed = valueStr.trim();
  if (!trimmed) return null;
  const value = parseInt(trimmed, 10);
  if (Number.isNaN(value)) return null;
  return { op, value };
}

export function parseQueryToFilters(query: string): {
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

  const tokens: string[] = [];
  let current = "";
  let inQuotes = false;
  for (const character of query) {
    if (character === '"') {
      inQuotes = !inQuotes;
    } else if ((character === " " || character === "\t") && !inQuotes) {
      if (current) {
        tokens.push(current);
        current = "";
      }
    } else {
      current += character;
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
      const expression = token.slice(8);
      if (expression.includes("..")) {
        const [start, end] = expression.split("..");
        if (start) filters.createdAfter = start;
        if (end) filters.createdBefore = end;
      } else if (expression.startsWith(">")) {
        filters.createdAfter = expression.slice(1);
      } else if (expression.startsWith("<")) {
        filters.createdBefore = expression.slice(1);
      } else {
        filters.createdAfter = expression;
        filters.createdBefore = expression;
      }
    } else if (token) {
      textTerms.push(token);
    }
  }

  return { textTerms, filters };
}

export function buildQueryFromFilters(
  textTerms: string[],
  filters: FilterState,
): string {
  const parts: string[] = [];

  for (const term of textTerms) {
    parts.push(term.includes(" ") ? `"${term}"` : term);
  }

  for (const tag of filters.tags) {
    parts.push(tag.includes(" ") ? `tag:"${tag}"` : `tag:${tag}`);
  }

  if (filters.source) {
    parts.push(
      filters.source.includes(" ")
        ? `source:"${filters.source}"`
        : `source:${filters.source}`,
    );
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
    parts.push(
      filters.createdAfter === filters.createdBefore
        ? `created:${filters.createdAfter}`
        : `created:${filters.createdAfter}..${filters.createdBefore}`,
    );
  } else if (filters.createdAfter) {
    parts.push(`created:>${filters.createdAfter}`);
  } else if (filters.createdBefore) {
    parts.push(`created:<${filters.createdBefore}`);
  }

  return parts.join(" ");
}

export function countActiveFilters(filters: FilterState): number {
  let count = filters.tags.length;
  if (filters.source) count++;
  if (filters.photos !== "any") count++;
  if (filters.createdAfter || filters.createdBefore) count++;
  if (filters.photoSize) count++;
  if (filters.photoDim) count++;
  return count;
}
