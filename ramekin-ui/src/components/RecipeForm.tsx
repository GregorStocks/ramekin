import { Show, Index, For } from "solid-js";
import type { Accessor, Setter } from "solid-js";
import type { SetStoreFunction } from "solid-js/store";
import { A } from "@solidjs/router";
import TagInput from "./TagInput";
import StarRating from "./StarRating";
import PhotoThumbnail from "./PhotoThumbnail";
import type { Ingredient } from "ramekin-client";
import {
  addIngredient,
  removeIngredient,
  updateIngredientItem,
  updateIngredientNote,
  addAlternativeMeasurement,
  removeMeasurement,
  updateMeasurementAmount,
  updateMeasurementUnit,
  getMeasurementAmount,
  getMeasurementUnit,
} from "../utils/recipeFormHelpers";

export interface RecipeFormProps {
  // Form data signals
  title: Accessor<string>;
  setTitle: Setter<string>;
  description: Accessor<string>;
  setDescription: Setter<string>;
  instructions: Accessor<string>;
  setInstructions: Setter<string>;
  sourceUrl: Accessor<string>;
  setSourceUrl: Setter<string>;
  sourceName: Accessor<string>;
  setSourceName: Setter<string>;
  tags: Accessor<string[]>;
  setTags: Setter<string[]>;
  servings: Accessor<string>;
  setServings: Setter<string>;
  prepTime: Accessor<string>;
  setPrepTime: Setter<string>;
  cookTime: Accessor<string>;
  setCookTime: Setter<string>;
  totalTime: Accessor<string>;
  setTotalTime: Setter<string>;
  rating: Accessor<number | null>;
  setRating: Setter<number | null>;
  difficulty: Accessor<string>;
  setDifficulty: Setter<string>;
  nutritionalInfo: Accessor<string>;
  setNutritionalInfo: Setter<string>;
  notes: Accessor<string>;
  setNotes: Setter<string>;

  // Ingredient store
  ingredients: Ingredient[];
  setIngredients: SetStoreFunction<Ingredient[]>;

  // Photo management
  photoIds: Accessor<string[]>;
  onPhotoUpload: (e: Event) => Promise<void>;
  onPhotoRemove: (photoId: string) => void;
  uploading: Accessor<boolean>;

  // Form state
  saving: Accessor<boolean>;
  error: Accessor<string | null>;

  // Submit handling
  onSubmit: (e: Event) => void;
  submitLabel: string;
  submitLabelSaving: string;

  // Cancel handling
  cancelHref: string;

  // Auth token for photo display
  token: Accessor<string | null | undefined>;
}

export default function RecipeForm(props: RecipeFormProps) {
  return (
    <form onSubmit={props.onSubmit}>
      <div class="form-group">
        <label for="title">Title *</label>
        <input
          id="title"
          type="text"
          value={props.title()}
          onInput={(e) => props.setTitle(e.currentTarget.value)}
          required
        />
      </div>

      <div class="form-group">
        <label for="description">Description</label>
        <textarea
          id="description"
          value={props.description()}
          onInput={(e) => props.setDescription(e.currentTarget.value)}
          rows={2}
        />
      </div>

      <div class="form-row-4">
        <div class="form-group">
          <label for="servings">Servings</label>
          <input
            id="servings"
            type="text"
            value={props.servings()}
            onInput={(e) => props.setServings(e.currentTarget.value)}
            placeholder="e.g., 4"
          />
        </div>
        <div class="form-group">
          <label for="prepTime">Prep Time</label>
          <input
            id="prepTime"
            type="text"
            value={props.prepTime()}
            onInput={(e) => props.setPrepTime(e.currentTarget.value)}
            placeholder="e.g., 15 min"
          />
        </div>
        <div class="form-group">
          <label for="cookTime">Cook Time</label>
          <input
            id="cookTime"
            type="text"
            value={props.cookTime()}
            onInput={(e) => props.setCookTime(e.currentTarget.value)}
            placeholder="e.g., 30 min"
          />
        </div>
        <div class="form-group">
          <label for="totalTime">Total Time</label>
          <input
            id="totalTime"
            type="text"
            value={props.totalTime()}
            onInput={(e) => props.setTotalTime(e.currentTarget.value)}
            placeholder="e.g., 45 min"
          />
        </div>
      </div>

      <div class="form-row">
        <div class="form-group-rating">
          <label>Rating</label>
          <div class="rating-input-wrapper">
            <StarRating rating={props.rating()} onRate={props.setRating} />
            <Show when={props.rating() !== null}>
              <button
                type="button"
                class="rating-clear"
                onClick={() => props.setRating(null)}
              >
                Clear
              </button>
            </Show>
          </div>
        </div>
        <div class="form-group">
          <label for="difficulty">Difficulty</label>
          <input
            id="difficulty"
            type="text"
            value={props.difficulty()}
            onInput={(e) => props.setDifficulty(e.currentTarget.value)}
            placeholder="e.g., Easy, Medium, Hard"
          />
        </div>
      </div>

      <div class="form-section">
        <div class="section-header">
          <label>Ingredients</label>
          <button
            type="button"
            class="btn btn-small"
            onClick={() =>
              addIngredient(props.ingredients, props.setIngredients)
            }
          >
            + Add
          </button>
        </div>
        <Index each={props.ingredients}>
          {(ing, index) => (
            <div class="ingredient-entry">
              <div class="ingredient-row">
                <input
                  type="text"
                  placeholder="Amount"
                  value={getMeasurementAmount(ing(), 0)}
                  onInput={(e) =>
                    updateMeasurementAmount(
                      index,
                      0,
                      e.currentTarget.value,
                      props.setIngredients,
                    )
                  }
                  class="input-amount"
                />
                <input
                  type="text"
                  placeholder="Unit"
                  value={getMeasurementUnit(ing(), 0)}
                  onInput={(e) =>
                    updateMeasurementUnit(
                      index,
                      0,
                      e.currentTarget.value,
                      props.setIngredients,
                    )
                  }
                  class="input-unit"
                />
                <input
                  type="text"
                  placeholder="Ingredient *"
                  value={ing().item}
                  onInput={(e) =>
                    updateIngredientItem(
                      index,
                      e.currentTarget.value,
                      props.setIngredients,
                    )
                  }
                  class="input-item"
                />
                <input
                  type="text"
                  placeholder="Note"
                  value={ing().note || ""}
                  onInput={(e) =>
                    updateIngredientNote(
                      index,
                      e.currentTarget.value,
                      props.setIngredients,
                    )
                  }
                  class="input-note"
                />
                <button
                  type="button"
                  class="btn btn-small btn-add-alt"
                  onClick={() =>
                    addAlternativeMeasurement(index, props.setIngredients)
                  }
                  title="Add alternative measurement"
                >
                  +
                </button>
                <button
                  type="button"
                  class="btn btn-small btn-remove"
                  onClick={() => removeIngredient(index, props.setIngredients)}
                >
                  &times;
                </button>
              </div>
              <Show when={ing().measurements.length > 1}>
                <div class="alt-measurements">
                  <Index each={ing().measurements.slice(1)}>
                    {(_, mIndex) => (
                      <div class="alt-measurement-row">
                        <span class="alt-label">Alt:</span>
                        <input
                          type="text"
                          placeholder="Amount"
                          value={getMeasurementAmount(ing(), mIndex + 1)}
                          onInput={(e) =>
                            updateMeasurementAmount(
                              index,
                              mIndex + 1,
                              e.currentTarget.value,
                              props.setIngredients,
                            )
                          }
                          class="input-amount"
                        />
                        <input
                          type="text"
                          placeholder="Unit"
                          value={getMeasurementUnit(ing(), mIndex + 1)}
                          onInput={(e) =>
                            updateMeasurementUnit(
                              index,
                              mIndex + 1,
                              e.currentTarget.value,
                              props.setIngredients,
                            )
                          }
                          class="input-unit"
                        />
                        <button
                          type="button"
                          class="btn btn-small btn-remove"
                          onClick={() =>
                            removeMeasurement(
                              index,
                              mIndex + 1,
                              props.setIngredients,
                            )
                          }
                          title="Remove alternative measurement"
                        >
                          &times;
                        </button>
                      </div>
                    )}
                  </Index>
                </div>
              </Show>
              <Show when={ing().raw}>
                <div class="ingredient-raw">
                  <span class="raw-label">Raw:</span> {ing().raw}
                </div>
              </Show>
            </div>
          )}
        </Index>
      </div>

      <div class="form-group">
        <label for="instructions">Instructions *</label>
        <textarea
          id="instructions"
          value={props.instructions()}
          onInput={(e) => props.setInstructions(e.currentTarget.value)}
          rows={8}
          required
        />
      </div>

      <div class="form-row">
        <div class="form-group">
          <label for="sourceUrl">Source URL</label>
          <input
            id="sourceUrl"
            type="url"
            value={props.sourceUrl()}
            onInput={(e) => props.setSourceUrl(e.currentTarget.value)}
            placeholder="https://..."
          />
        </div>
        <div class="form-group">
          <label for="sourceName">Source Name</label>
          <input
            id="sourceName"
            type="text"
            value={props.sourceName()}
            onInput={(e) => props.setSourceName(e.currentTarget.value)}
            placeholder="e.g., Grandma's cookbook"
          />
        </div>
      </div>

      <div class="form-group">
        <label for="tags">Tags</label>
        <TagInput
          id="tags"
          tags={props.tags}
          onTagsChange={props.setTags}
          placeholder="e.g., dinner, easy, vegetarian"
        />
      </div>

      <div class="form-group">
        <label for="notes">Notes</label>
        <textarea
          id="notes"
          value={props.notes()}
          onInput={(e) => props.setNotes(e.currentTarget.value)}
          rows={3}
          placeholder="Additional notes, tips, or variations..."
        />
      </div>

      <div class="form-group">
        <label for="nutritionalInfo">Nutritional Info</label>
        <textarea
          id="nutritionalInfo"
          value={props.nutritionalInfo()}
          onInput={(e) => props.setNutritionalInfo(e.currentTarget.value)}
          rows={2}
          placeholder="Calories, protein, carbs, etc."
        />
      </div>

      <div class="form-section">
        <div class="section-header">
          <label>Photos</label>
          <label class="btn btn-small">
            {props.uploading() ? "Uploading..." : "+ Add Photo"}
            <input
              type="file"
              accept="image/*"
              onChange={props.onPhotoUpload}
              disabled={props.uploading()}
              style={{ display: "none" }}
            />
          </label>
        </div>
        <Show when={props.photoIds().length > 0}>
          <div class="photo-grid">
            <For each={props.photoIds()}>
              {(photoId) => (
                <PhotoThumbnail
                  photoId={photoId}
                  token={props.token() ?? ""}
                  onRemove={() => props.onPhotoRemove(photoId)}
                />
              )}
            </For>
          </div>
        </Show>
        <Show when={props.photoIds().length === 0}>
          <p class="empty-photos">No photos yet</p>
        </Show>
      </div>

      <Show when={props.error()}>
        <div class="error">{props.error()}</div>
      </Show>

      <div class="form-actions">
        <A href={props.cancelHref} class="btn">
          Cancel
        </A>
        <button type="submit" class="btn btn-primary" disabled={props.saving()}>
          {props.saving() ? props.submitLabelSaving : props.submitLabel}
        </button>
      </div>
    </form>
  );
}
