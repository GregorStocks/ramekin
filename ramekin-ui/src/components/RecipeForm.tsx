import { Show, Index, For, createSignal } from "solid-js";
import type { Accessor, Setter } from "solid-js";
import type { SetStoreFunction } from "solid-js/store";
import { A } from "@solidjs/router";
import {
  DragDropProvider,
  DragDropSensors,
  DragOverlay,
  SortableProvider,
  createSortable,
  closestCenter,
} from "@thisbeyond/solid-dnd";
import type { DragEvent } from "@thisbeyond/solid-dnd";
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
  updateIngredientSection,
  addIngredientWithSection,
  groupIngredientsBySection,
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

/** Sortable ingredient row component */
function SortableIngredient(props: {
  ing: Ingredient;
  index: number;
  setIngredients: SetStoreFunction<Ingredient[]>;
}) {
  const sortable = createSortable(props.index);
  return (
    <div
      ref={sortable.ref}
      class="ingredient-entry"
      classList={{ "is-dragging": sortable.isActiveDraggable }}
      style={{ opacity: sortable.isActiveDraggable ? 0.5 : 1 }}
    >
      <div class="ingredient-row" {...sortable.dragActivators}>
        <span class="drag-handle" title="Drag to reorder">
          ⋮⋮
        </span>
        <input
          type="text"
          placeholder="Amount"
          value={getMeasurementAmount(props.ing, 0)}
          onInput={(e) =>
            updateMeasurementAmount(
              props.index,
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
          value={getMeasurementUnit(props.ing, 0)}
          onInput={(e) =>
            updateMeasurementUnit(
              props.index,
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
          value={props.ing.item}
          onInput={(e) =>
            updateIngredientItem(
              props.index,
              e.currentTarget.value,
              props.setIngredients,
            )
          }
          class="input-item"
        />
        <input
          type="text"
          placeholder="Note"
          value={props.ing.note || ""}
          onInput={(e) =>
            updateIngredientNote(
              props.index,
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
            addAlternativeMeasurement(props.index, props.setIngredients)
          }
          title="Add alternative measurement"
        >
          +
        </button>
        <button
          type="button"
          class="btn btn-small btn-remove"
          onClick={() => removeIngredient(props.index, props.setIngredients)}
        >
          &times;
        </button>
      </div>
      <Show when={props.ing.measurements.length > 1}>
        <div class="alt-measurements">
          <Index each={props.ing.measurements.slice(1)}>
            {(_, mIndex) => (
              <div class="alt-measurement-row">
                <span class="alt-label">Alt:</span>
                <input
                  type="text"
                  placeholder="Amount"
                  value={getMeasurementAmount(props.ing, mIndex + 1)}
                  onInput={(e) =>
                    updateMeasurementAmount(
                      props.index,
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
                  value={getMeasurementUnit(props.ing, mIndex + 1)}
                  onInput={(e) =>
                    updateMeasurementUnit(
                      props.index,
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
                      props.index,
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
      <Show when={props.ing.raw}>
        <div class="ingredient-raw">
          <span class="raw-label">Raw:</span> {props.ing.raw}
        </div>
      </Show>
    </div>
  );
}

/** Ingredients section with drag-and-drop and section support */
function IngredientsSection(props: {
  ingredients: Ingredient[];
  setIngredients: SetStoreFunction<Ingredient[]>;
}) {
  const [newSectionName, setNewSectionName] = createSignal("");

  const ingredientIds = () => props.ingredients.map((_, i) => i);

  const onDragEnd = (event: DragEvent) => {
    const { draggable, droppable } = event;
    if (draggable && droppable && draggable.id !== droppable.id) {
      const fromIndex = draggable.id as number;
      const toIndex = droppable.id as number;
      // Move ingredient and inherit section from the target position
      const targetSection = props.ingredients[toIndex]?.section;
      const updated = [...props.ingredients];
      const [moved] = updated.splice(fromIndex, 1);
      moved.section = targetSection;
      updated.splice(toIndex, 0, moved);
      props.setIngredients(updated);
    }
  };

  const addSection = () => {
    const name = newSectionName().trim();
    if (name) {
      addIngredientWithSection(props.ingredients, props.setIngredients, name);
      setNewSectionName("");
    }
  };

  const groups = () => groupIngredientsBySection(props.ingredients);

  return (
    <div class="form-section">
      <div class="section-header">
        <label>Ingredients</label>
        <div class="section-header-actions">
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
      </div>

      <DragDropProvider onDragEnd={onDragEnd} collisionDetector={closestCenter}>
        <DragDropSensors />
        <SortableProvider ids={ingredientIds()}>
          <For each={groups()}>
            {(group) => (
              <div class="ingredient-section-group">
                <Show when={group.section !== null}>
                  <div class="ingredient-section-header-row">
                    <input
                      type="text"
                      class="section-name-input"
                      value={group.section || ""}
                      placeholder="Section name"
                      onInput={(e) => {
                        // Update section name for all ingredients in this group
                        const newName = e.currentTarget.value || undefined;
                        for (let i = 0; i < group.ingredients.length; i++) {
                          updateIngredientSection(
                            group.startIndex + i,
                            newName,
                            props.setIngredients,
                          );
                        }
                      }}
                    />
                    <button
                      type="button"
                      class="btn btn-small"
                      onClick={() =>
                        addIngredientWithSection(
                          props.ingredients,
                          props.setIngredients,
                          group.section || "",
                        )
                      }
                      title="Add ingredient to this section"
                    >
                      +
                    </button>
                  </div>
                </Show>
                <For each={group.ingredients}>
                  {(ing, localIndex) => (
                    <SortableIngredient
                      ing={ing}
                      index={group.startIndex + localIndex()}
                      setIngredients={props.setIngredients}
                    />
                  )}
                </For>
              </div>
            )}
          </For>
        </SortableProvider>
        <DragOverlay>
          {(draggable) => {
            const ing = props.ingredients[draggable?.id as number];
            return ing ? (
              <div class="ingredient-entry drag-overlay">
                <div class="ingredient-row">
                  <span class="drag-handle">⋮⋮</span>
                  <span class="input-amount">
                    {getMeasurementAmount(ing, 0)}
                  </span>
                  <span class="input-unit">{getMeasurementUnit(ing, 0)}</span>
                  <span class="input-item">{ing.item}</span>
                </div>
              </div>
            ) : null;
          }}
        </DragOverlay>
      </DragDropProvider>

      <div class="add-section-row">
        <input
          type="text"
          placeholder="New section name..."
          value={newSectionName()}
          onInput={(e) => setNewSectionName(e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              addSection();
            }
          }}
          class="section-name-input"
        />
        <button
          type="button"
          class="btn btn-small"
          onClick={addSection}
          disabled={!newSectionName().trim()}
        >
          + Add Section
        </button>
      </div>
    </div>
  );
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

      <IngredientsSection
        ingredients={props.ingredients}
        setIngredients={props.setIngredients}
      />

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
