import { createEffect, createMemo, createSignal, onCleanup } from "solid-js";
import type { Accessor } from "solid-js";

import { groupTags } from "../utils/tagHierarchy";
import {
  buildQueryFromFilters,
  buildThresholdFromInput,
  countActiveFilters,
  parseQueryToFilters,
  type FilterState,
} from "../pages/cookbook/query";

interface UseCookbookFiltersOptions {
  query: Accessor<string>;
  availableTags: Accessor<string[]>;
  setQuery: (query: string) => void;
}

export function useCookbookFilters(options: UseCookbookFiltersOptions) {
  const [mobileFiltersOpen, setMobileFiltersOpen] = createSignal(false);
  const [sourceInput, setSourceInput] = createSignal("");
  const [photoSizeOp, setPhotoSizeOp] = createSignal<"<" | ">">("<");
  const [photoSizeInput, setPhotoSizeInput] = createSignal("");
  const [photoDimOp, setPhotoDimOp] = createSignal<"<" | ">">("<");
  const [photoDimInput, setPhotoDimInput] = createSignal("");
  const [caloriesMinInput, setCaloriesMinInput] = createSignal("");
  const [caloriesMaxInput, setCaloriesMaxInput] = createSignal("");

  const parsedQuery = createMemo(() => parseQueryToFilters(options.query()));
  const currentFilters = () => parsedQuery().filters;
  const currentTextTerms = () => parsedQuery().textTerms;
  const sortedTags = () =>
    [...options.availableTags()].sort((a, b) =>
      a.localeCompare(b, undefined, { sensitivity: "base" }),
    );
  const groupedTags = () => groupTags(sortedTags());
  const hasTextQuery = () => currentTextTerms().length > 0;
  const activeFilterCount = () => countActiveFilters(currentFilters());

  createEffect(() => {
    const filters = currentFilters();
    setSourceInput(filters.source);
    setCaloriesMinInput(filters.caloriesMin);
    setCaloriesMaxInput(filters.caloriesMax);
    if (filters.photoSize) {
      setPhotoSizeOp(filters.photoSize.op);
      setPhotoSizeInput(String(filters.photoSize.value));
    } else {
      setPhotoSizeInput("");
    }
    if (filters.photoDim) {
      setPhotoDimOp(filters.photoDim.op);
      setPhotoDimInput(String(filters.photoDim.value));
    } else {
      setPhotoDimInput("");
    }
  });

  createEffect(() => {
    if (!mobileFiltersOpen()) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      // Let an open tag popover consume Escape before the drawer does.
      if (document.querySelector(".cookbook-tag-popover:popover-open")) return;
      setMobileFiltersOpen(false);
    };
    document.addEventListener("keydown", onKey);
    document.body.style.overflow = "hidden";
    onCleanup(() => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = "";
    });
  });

  const patchFilters = (patch: Partial<FilterState>) => {
    const next: FilterState = {
      ...currentFilters(),
      source: sourceInput().trim(),
      photoSize: buildThresholdFromInput(photoSizeInput(), photoSizeOp()),
      photoDim: buildThresholdFromInput(photoDimInput(), photoDimOp()),
      caloriesMin: caloriesMinInput().trim(),
      caloriesMax: caloriesMaxInput().trim(),
      ...patch,
    };
    options.setQuery(buildQueryFromFilters(currentTextTerms(), next));
  };

  const clearFilters = () => {
    setSourceInput("");
    setPhotoSizeInput("");
    setPhotoDimInput("");
    setCaloriesMinInput("");
    setCaloriesMaxInput("");
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
      tags: tags.includes(tag)
        ? tags.filter((item) => item !== tag)
        : [...tags, tag],
    });
  };

  const flushLocalInputs = () => patchFilters({});

  const handleOpChange = (
    setOp: (op: "<" | ">") => void,
    input: Accessor<string>,
    op: "<" | ">",
  ) => {
    setOp(op);
    if (input().trim()) flushLocalInputs();
  };

  const handleFlushKeyDown = (
    event: KeyboardEvent & { currentTarget: HTMLInputElement },
  ) => {
    if (event.key === "Enter") {
      event.preventDefault();
      flushLocalInputs();
      event.currentTarget.blur();
    }
  };

  return {
    mobileFiltersOpen,
    setMobileFiltersOpen,
    sourceInput,
    setSourceInput,
    photoSizeOp,
    setPhotoSizeOp,
    photoSizeInput,
    setPhotoSizeInput,
    photoDimOp,
    setPhotoDimOp,
    photoDimInput,
    setPhotoDimInput,
    caloriesMinInput,
    setCaloriesMinInput,
    caloriesMaxInput,
    setCaloriesMaxInput,
    currentFilters,
    sortedTags,
    groupedTags,
    hasTextQuery,
    activeFilterCount,
    patchFilters,
    clearFilters,
    toggleTag,
    flushLocalInputs,
    handleOpChange,
    handleFlushKeyDown,
  };
}

export type CookbookFiltersState = ReturnType<typeof useCookbookFilters>;
