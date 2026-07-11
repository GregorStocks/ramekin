import { For, Show } from "solid-js";
import type { Accessor, Setter } from "solid-js";

import type { SortOption } from "../../pages/cookbook/query";
import type { Density } from "../../pages/cookbook/presentation";

interface CookbookToolbarProps {
  searchInput: Accessor<string>;
  updateSearchQuery: (value: string) => void;
  handleSearch: (event?: Event) => void;
  clearSearch: () => void;
  activeFilterCount: Accessor<number>;
  openFilters: () => void;
  hasTextQuery: Accessor<boolean>;
  sortOption: Accessor<SortOption>;
  setSortOption: (sort: SortOption) => void;
  goToRandomRecipe: () => void;
  total: Accessor<number>;
  bulkMode: Accessor<boolean>;
  toggleBulkMode: () => void;
  density: Accessor<Density>;
  setDensity: Setter<Density>;
}

export default function CookbookToolbar(props: CookbookToolbarProps) {
  return (
    <div class="cookbook-utility-bar">
      <form class="search-bar" onSubmit={props.handleSearch}>
        <input
          type="text"
          class="search-input"
          placeholder="Search recipes..."
          value={props.searchInput()}
          onInput={(event) =>
            props.updateSearchQuery(event.currentTarget.value)
          }
        />
        <Show when={props.searchInput()}>
          <button
            type="button"
            class="search-clear"
            onClick={props.clearSearch}
          >
            &times;
          </button>
        </Show>
      </form>
      <button
        type="button"
        class="filter-button cookbook-mobile-filters-toggle"
        onClick={props.openFilters}
        classList={{ active: props.activeFilterCount() > 0 }}
        aria-label="Open filters"
      >
        Filters
        <Show when={props.activeFilterCount() > 0}>
          <span class="filter-badge">{props.activeFilterCount()}</span>
        </Show>
      </button>
      <select
        class="sort-select"
        value={props.hasTextQuery() ? "best" : props.sortOption()}
        disabled={props.hasTextQuery()}
        title={
          props.hasTextQuery()
            ? "Search results are ranked by relevance"
            : undefined
        }
        onChange={(event) =>
          props.setSortOption(event.currentTarget.value as SortOption)
        }
      >
        <Show when={props.hasTextQuery()}>
          <option value="best">Best match</option>
        </Show>
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
        onClick={props.goToRandomRecipe}
        disabled={props.total() === 0}
      >
        Random
      </button>
      <button
        type="button"
        class="filter-button"
        onClick={props.toggleBulkMode}
        classList={{ active: props.bulkMode() }}
      >
        {props.bulkMode() ? "Done" : "Select"}
      </button>
      <div class="density-toggle" role="group" aria-label="Recipe density">
        <For each={["card", "compact", "list"] as const}>
          {(mode) => (
            <button
              type="button"
              class="density-toggle-button"
              classList={{ active: props.density() === mode }}
              aria-pressed={props.density() === mode}
              onClick={() => props.setDensity(mode)}
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
  );
}
