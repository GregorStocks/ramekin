import { Show, For } from "solid-js";
import type { Accessor, JSX } from "solid-js";
import { A } from "@solidjs/router";
import TagInput from "./TagInput";
import StarRating from "./StarRating";
import PhotoThumbnail from "./PhotoThumbnail";
import IngredientEditor from "./IngredientEditor";
import type { RecipeFormState } from "../utils/recipeFormState";

export interface RecipeFormProps {
  form: RecipeFormState;
  onSubmit: (e: Event) => void;
  submitLabel: string;
  submitLabelSaving: string;
  cancelHref: string;
  token: Accessor<string | null | undefined>;
  ingredientEditor?: JSX.Element;
}

export default function RecipeForm(props: RecipeFormProps) {
  let photoUploadInput: HTMLInputElement | undefined;
  return (
    <form onSubmit={props.onSubmit}>
      <div class="form-group">
        <label for="title">Title *</label>
        <input
          id="title"
          type="text"
          value={props.form.title()}
          onInput={(e) => props.form.setTitle(e.currentTarget.value)}
          required
        />
      </div>

      <div class="form-group">
        <label for="description">Description</label>
        <textarea
          id="description"
          value={props.form.description()}
          onInput={(e) => props.form.setDescription(e.currentTarget.value)}
          rows={2}
        />
      </div>

      <div class="form-row-4">
        <div class="form-group">
          <label for="servings">Servings</label>
          <input
            id="servings"
            type="text"
            value={props.form.servings()}
            onInput={(e) => props.form.setServings(e.currentTarget.value)}
            placeholder="e.g., 4"
          />
        </div>
        <div class="form-group">
          <label for="prepTime">Prep Time</label>
          <input
            id="prepTime"
            type="text"
            value={props.form.prepTime()}
            onInput={(e) => props.form.setPrepTime(e.currentTarget.value)}
            placeholder="e.g., 15 min"
          />
        </div>
        <div class="form-group">
          <label for="cookTime">Cook Time</label>
          <input
            id="cookTime"
            type="text"
            value={props.form.cookTime()}
            onInput={(e) => props.form.setCookTime(e.currentTarget.value)}
            placeholder="e.g., 30 min"
          />
        </div>
        <div class="form-group">
          <label for="totalTime">Total Time</label>
          <input
            id="totalTime"
            type="text"
            value={props.form.totalTime()}
            onInput={(e) => props.form.setTotalTime(e.currentTarget.value)}
            placeholder="e.g., 45 min"
          />
        </div>
      </div>

      <div class="form-row">
        <div class="form-group-rating">
          <label>Rating</label>
          <div class="rating-input-wrapper">
            <StarRating
              rating={props.form.rating()}
              onRate={props.form.setRating}
            />
            <Show when={props.form.rating() !== null}>
              <button
                type="button"
                class="rating-clear"
                onClick={() => props.form.setRating(null)}
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
            value={props.form.difficulty()}
            onInput={(e) => props.form.setDifficulty(e.currentTarget.value)}
            placeholder="e.g., Easy, Medium, Hard"
          />
        </div>
      </div>

      <Show
        when={props.ingredientEditor}
        fallback={
          <IngredientEditor
            rows={props.form.ingredientRows}
            setRows={props.form.setIngredientRows}
          />
        }
      >
        {props.ingredientEditor}
      </Show>

      <div class="form-group">
        <label for="instructions">Instructions *</label>
        <textarea
          id="instructions"
          value={props.form.instructions()}
          onInput={(e) => props.form.setInstructions(e.currentTarget.value)}
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
            value={props.form.sourceUrl()}
            onInput={(e) => props.form.setSourceUrl(e.currentTarget.value)}
            placeholder="https://..."
          />
        </div>
        <div class="form-group">
          <label for="sourceName">Source Name</label>
          <input
            id="sourceName"
            type="text"
            value={props.form.sourceName()}
            onInput={(e) => props.form.setSourceName(e.currentTarget.value)}
            placeholder="e.g., Grandma's cookbook"
          />
        </div>
      </div>

      <div class="form-group">
        <label for="tags">Tags</label>
        <TagInput
          id="tags"
          tags={props.form.tags}
          onTagsChange={props.form.setTags}
          placeholder="e.g., dinner, easy, vegetarian"
        />
      </div>

      <div class="form-group">
        <label for="notes">Notes</label>
        <textarea
          id="notes"
          value={props.form.notes()}
          onInput={(e) => props.form.setNotes(e.currentTarget.value)}
          rows={3}
          placeholder="Additional notes, tips, or variations..."
        />
      </div>

      <div class="form-group">
        <label for="nutritionalInfo">Nutritional Info</label>
        <textarea
          id="nutritionalInfo"
          value={props.form.nutritionalInfo()}
          onInput={(e) => props.form.setNutritionalInfo(e.currentTarget.value)}
          rows={2}
          placeholder="Calories, protein, carbs, etc."
        />
      </div>

      <div class="form-section">
        <div class="section-header">
          <label>Photos</label>
          <div class="section-header-actions">
            <span class="photo-paste-hint">or paste from clipboard</span>
            <button
              type="button"
              class="btn btn-small"
              onClick={() => photoUploadInput?.click()}
              disabled={props.form.uploading()}
            >
              {props.form.uploading() ? "Uploading..." : "+ Add Photo"}
            </button>
            <input
              ref={photoUploadInput}
              class="photo-upload-input"
              type="file"
              accept="image/*"
              aria-label="Recipe photo"
              onChange={props.form.onPhotoUpload}
              disabled={props.form.uploading()}
            />
          </div>
        </div>
        <Show when={props.form.photoIds().length > 0}>
          <div class="photo-grid">
            <For each={props.form.photoIds()}>
              {(photoId) => (
                <PhotoThumbnail
                  photoId={photoId}
                  token={props.token() ?? ""}
                  onRemove={() => props.form.removePhoto(photoId)}
                />
              )}
            </For>
          </div>
        </Show>
        <Show when={props.form.photoIds().length === 0}>
          <p class="empty-photos">No photos yet</p>
        </Show>
      </div>

      <Show when={props.form.error()}>
        <div class="error">{props.form.error()}</div>
      </Show>

      <div class="form-actions">
        <A href={props.cancelHref} class="btn">
          Cancel
        </A>
        <button
          type="submit"
          class="btn btn-primary"
          disabled={props.form.saving()}
        >
          {props.form.saving() ? props.submitLabelSaving : props.submitLabel}
        </button>
      </div>
    </form>
  );
}
