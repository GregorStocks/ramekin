import { For, Show } from "solid-js";

import type { CookbookFiltersState } from "../../hooks/useCookbookFilters";
import { parseTag } from "../../utils/tagHierarchy";

interface CookbookFiltersProps {
  state: CookbookFiltersState;
}

export default function CookbookFilters(props: CookbookFiltersProps) {
  const state = props.state;

  const renderThreshold = (
    label: string,
    placeholder: string,
    op: () => "<" | ">",
    setOp: (op: "<" | ">") => void,
    input: () => string,
    setInput: (value: string) => void,
  ) => (
    <div class="cookbook-filter-section">
      <div class="cookbook-filter-label">{label}</div>
      <div class="cookbook-filter-threshold-row">
        <select
          class="cookbook-filter-input cookbook-filter-input-op"
          value={op()}
          onChange={(event) =>
            state.handleOpChange(
              setOp,
              input,
              event.currentTarget.value as "<" | ">",
            )
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
          onInput={(event) => setInput(event.currentTarget.value)}
          onBlur={state.flushLocalInputs}
          onKeyDown={state.handleFlushKeyDown}
        />
      </div>
    </div>
  );

  return (
    <>
      <aside
        class="cookbook-sidebar"
        classList={{ "cookbook-sidebar-open": state.mobileFiltersOpen() }}
        aria-label="Filters"
      >
        <div class="cookbook-sidebar-header">
          <h3 class="cookbook-sidebar-title">Filters</h3>
          <button
            type="button"
            class="cookbook-sidebar-close"
            onClick={() => state.setMobileFiltersOpen(false)}
            aria-label="Close filters"
          >
            ✕
          </button>
        </div>

        <div class="cookbook-filter-section">
          <div class="cookbook-filter-label">Tags</div>
          <Show
            when={state.sortedTags().length > 0}
            fallback={<span class="cookbook-filter-empty">No tags yet</span>}
          >
            <div class="cookbook-filter-tag-groups">
              <For each={state.groupedTags()}>
                {(group) => (
                  <details class="cookbook-filter-tag-group" open>
                    <summary>
                      {group.namespace ?? "Uncategorized"}
                      <span class="cookbook-filter-tag-group-count">
                        {` (${group.tags.length})`}
                      </span>
                    </summary>
                    <div class="cookbook-filter-chips">
                      <For each={group.tags}>
                        {(tag) => {
                          const parsed = parseTag(tag);
                          const selected = () =>
                            state.currentFilters().tags.includes(tag);
                          return (
                            <button
                              type="button"
                              class="filter-chip"
                              classList={{ "filter-chip-active": selected() }}
                              aria-pressed={selected()}
                              onClick={() => state.toggleTag(tag)}
                            >
                              <Show when={parsed.namespace}>
                                <span class="tag-chip-ns">
                                  {parsed.namespace}:
                                </span>
                              </Show>
                              {parsed.value}
                            </button>
                          );
                        }}
                      </For>
                    </div>
                  </details>
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
            value={state.sourceInput()}
            onInput={(event) => state.setSourceInput(event.currentTarget.value)}
            onBlur={state.flushLocalInputs}
            onKeyDown={state.handleFlushKeyDown}
          />
        </div>

        <div class="cookbook-filter-section">
          <div class="cookbook-filter-label">Photos</div>
          <div class="cookbook-filter-radio-group">
            <For
              each={
                [
                  ["any", "Any"],
                  ["has", "Has photos"],
                  ["no", "No photos"],
                ] as const
              }
            >
              {([value, label]) => (
                <label class="cookbook-filter-radio">
                  <input
                    type="radio"
                    name="photos"
                    checked={state.currentFilters().photos === value}
                    onChange={() => state.patchFilters({ photos: value })}
                  />
                  {label}
                </label>
              )}
            </For>
          </div>
        </div>

        {renderThreshold(
          "Photo file size (bytes)",
          "e.g. 100000",
          state.photoSizeOp,
          state.setPhotoSizeOp,
          state.photoSizeInput,
          state.setPhotoSizeInput,
        )}

        {renderThreshold(
          "Photo dimensions (min side, px)",
          "e.g. 600",
          state.photoDimOp,
          state.setPhotoDimOp,
          state.photoDimInput,
          state.setPhotoDimInput,
        )}

        <div class="cookbook-filter-section">
          <div class="cookbook-filter-label">Created</div>
          <div class="cookbook-filter-date-range">
            <input
              type="date"
              class="cookbook-filter-input"
              value={state.currentFilters().createdAfter}
              onInput={(event) =>
                state.patchFilters({ createdAfter: event.currentTarget.value })
              }
            />
            <span>to</span>
            <input
              type="date"
              class="cookbook-filter-input"
              value={state.currentFilters().createdBefore}
              onInput={(event) =>
                state.patchFilters({ createdBefore: event.currentTarget.value })
              }
            />
          </div>
        </div>

        <div class="cookbook-filter-actions">
          <button
            type="button"
            class="btn btn-small"
            onClick={state.clearFilters}
            disabled={state.activeFilterCount() === 0}
          >
            Clear all filters
          </button>
        </div>
      </aside>

      <Show when={state.mobileFiltersOpen()}>
        <div
          class="cookbook-sidebar-overlay"
          aria-hidden="true"
          onClick={() => state.setMobileFiltersOpen(false)}
        />
      </Show>
    </>
  );
}
