import { A } from "@solidjs/router";
import { For, Show } from "solid-js";
import type { Accessor } from "solid-js";
import type { RecipeSummary } from "ramekin-client";

import PhotoThumbnail from "../PhotoThumbnail";
import type { Density } from "../../pages/cookbook/presentation";
import { formatRelativeDate } from "../../pages/cookbook/presentation";
import { parseTag } from "../../utils/tagHierarchy";

const thumbnailSize = window.devicePixelRatio >= 2 ? 800 : 400;

interface CookbookRecipeGridProps {
  recipes: Accessor<RecipeSummary[]>;
  loading: Accessor<boolean>;
  loadingMore: Accessor<boolean>;
  hasMore: Accessor<boolean>;
  query: Accessor<string>;
  density: Accessor<Density>;
  bulkMode: Accessor<boolean>;
  selected: Accessor<Set<string>>;
  token: Accessor<string | null>;
  error: Accessor<string | null>;
  notice: Accessor<string | null>;
  clearSearch: () => void;
  toggleSelected: (id: string) => void;
}

export default function CookbookRecipeGrid(props: CookbookRecipeGridProps) {
  return (
    <div class="cookbook-content">
      <Show when={props.loading()}>
        <p class="loading">Loading recipes...</p>
      </Show>

      <Show when={props.error()}>
        <p class="error">{props.error()}</p>
      </Show>

      <Show when={props.notice()}>
        <p class="success">{props.notice()}</p>
      </Show>

      <Show
        when={
          !props.loading() && props.recipes().length === 0 && !props.query()
        }
      >
        <div class="empty-state">
          <div class="empty-state-icon">📖</div>
          <h3>Your cookbook is empty</h3>
          <p>Start building your collection by adding your first recipe.</p>
          <A href="/recipes/new" class="btn btn-primary">
            + Add Your First Recipe
          </A>
        </div>
      </Show>

      <Show
        when={!props.loading() && props.recipes().length === 0 && props.query()}
      >
        <div class="empty-state">
          <div class="empty-state-icon">🔍</div>
          <h3>No recipes found</h3>
          <p>Try a different search term or clear the search.</p>
          <button class="btn btn-primary" onClick={props.clearSearch}>
            Clear Search
          </button>
        </div>
      </Show>

      <Show when={!props.loading() && props.recipes().length > 0}>
        <div class="recipe-grid" data-density={props.density()}>
          <For each={props.recipes()}>
            {(recipe) => {
              const card = (
                <>
                  <Show
                    when={recipe.thumbnailPhotoId}
                    fallback={<div class="recipe-card-placeholder">🍽️</div>}
                  >
                    <PhotoThumbnail
                      photoId={recipe.thumbnailPhotoId!}
                      token={props.token()!}
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
                          {(tag) => {
                            const parsed = parseTag(tag);
                            return (
                              <span class="tag">
                                <Show when={parsed.namespace}>
                                  <span class="tag-chip-ns">
                                    {parsed.namespace}:
                                  </span>
                                </Show>
                                {parsed.value}
                              </span>
                            );
                          }}
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
                  when={props.bulkMode()}
                  fallback={
                    <A href={`/recipes/${recipe.id}`} class="recipe-card">
                      {card}
                    </A>
                  }
                >
                  <div
                    class="recipe-card recipe-card-selectable"
                    classList={{ selected: props.selected().has(recipe.id) }}
                    onClick={() => props.toggleSelected(recipe.id)}
                  >
                    <input
                      type="checkbox"
                      class="recipe-card-checkbox"
                      checked={props.selected().has(recipe.id)}
                      onClick={(event) => event.stopPropagation()}
                      onChange={() => props.toggleSelected(recipe.id)}
                    />
                    {card}
                  </div>
                </Show>
              );
            }}
          </For>
        </div>

        <Show when={props.loadingMore()}>
          <p
            class="loading"
            style={{ "text-align": "center", padding: "2rem" }}
          >
            Loading more recipes...
          </p>
        </Show>

        <Show when={!props.loadingMore() && !props.hasMore()}>
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
