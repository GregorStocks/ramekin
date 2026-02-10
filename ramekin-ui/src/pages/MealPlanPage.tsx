import { createSignal, createEffect, For, Show } from "solid-js";
import { A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import Modal from "../components/Modal";
import PhotoThumbnail from "../components/PhotoThumbnail";
import { extractApiError } from "../utils/recipeFormHelpers";
import type { MealPlanItem, RecipeSummary } from "ramekin-client";

const MEAL_TYPES = ["breakfast", "lunch", "dinner", "snack"] as const;
type MealTypeValue = (typeof MEAL_TYPES)[number];

const MEAL_TYPE_LABELS: Record<MealTypeValue, string> = {
  breakfast: "Breakfast",
  lunch: "Lunch",
  dinner: "Dinner",
  snack: "Snack",
};

function getMonday(d: Date): Date {
  const date = new Date(d);
  const day = date.getDay();
  const diff = date.getDate() - day + (day === 0 ? -6 : 1);
  date.setDate(diff);
  date.setHours(0, 0, 0, 0);
  return date;
}

function formatDateLocal(d: Date): string {
  const year = d.getFullYear();
  const month = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function formatDateUtc(d: Date): string {
  const year = d.getUTCFullYear();
  const month = String(d.getUTCMonth() + 1).padStart(2, "0");
  const day = String(d.getUTCDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function toApiDate(d: Date): Date {
  return new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()));
}

function formatDayHeader(d: Date): string {
  const days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const months = [
    "Jan",
    "Feb",
    "Mar",
    "Apr",
    "May",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Oct",
    "Nov",
    "Dec",
  ];
  return `${days[d.getDay()]} ${months[d.getMonth()]} ${d.getDate()}`;
}

export default function MealPlanPage() {
  const { getMealPlansApi, getRecipesApi, token } = useAuth();

  const [weekStart, setWeekStart] = createSignal(getMonday(new Date()));
  const [mealPlans, setMealPlans] = createSignal<MealPlanItem[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [pickerError, setPickerError] = createSignal<string | null>(null);

  // Recipe picker modal state
  const [pickerOpen, setPickerOpen] = createSignal(false);
  const [pickerDate, setPickerDate] = createSignal<Date | null>(null);
  const [pickerMealType, setPickerMealType] =
    createSignal<MealTypeValue | null>(null);
  const [recipes, setRecipes] = createSignal<RecipeSummary[]>([]);
  const [recipesLoading, setRecipesLoading] = createSignal(false);
  const [searchQuery, setSearchQuery] = createSignal("");

  // Delete confirmation
  const [deletingMealPlan, setDeletingMealPlan] =
    createSignal<MealPlanItem | null>(null);
  const [deleteLoading, setDeleteLoading] = createSignal(false);

  const weekDays = () => {
    const days: Date[] = [];
    const start = weekStart();
    for (let i = 0; i < 7; i++) {
      const d = new Date(start);
      d.setDate(start.getDate() + i);
      days.push(d);
    }
    return days;
  };

  const loadMealPlans = async () => {
    setLoading(true);
    setError(null);
    try {
      const days = weekDays();
      const response = await getMealPlansApi().listMealPlans({
        startDate: toApiDate(days[0]),
        endDate: toApiDate(days[6]),
      });
      setMealPlans(response.mealPlans);
    } catch (err) {
      const message = await extractApiError(err, "Failed to load meal plans");
      setError(message);
    } finally {
      setLoading(false);
    }
  };

  createEffect(() => {
    // Reload when week changes
    weekStart();
    loadMealPlans();
  });

  const getMealsForSlot = (date: Date, mealType: MealTypeValue) => {
    const dateStr = formatDateLocal(date);
    return mealPlans().filter(
      (mp) =>
        formatDateUtc(mp.mealDate) === dateStr && mp.mealType === mealType,
    );
  };

  const prevWeek = () => {
    const newStart = new Date(weekStart());
    newStart.setDate(newStart.getDate() - 7);
    setWeekStart(newStart);
  };

  const nextWeek = () => {
    const newStart = new Date(weekStart());
    newStart.setDate(newStart.getDate() + 7);
    setWeekStart(newStart);
  };

  const goToToday = () => {
    setWeekStart(getMonday(new Date()));
  };

  const openPicker = (date: Date, mealType: MealTypeValue) => {
    setPickerDate(date);
    setPickerMealType(mealType);
    setSearchQuery("");
    setPickerOpen(true);
    loadRecipes();
  };

  const loadRecipes = async () => {
    setPickerError(null);
    setRecipesLoading(true);
    try {
      const response = await getRecipesApi().listRecipes({
        limit: 50,
        q: searchQuery() || undefined,
      });
      setRecipes(response.recipes);
    } catch (err) {
      const message = await extractApiError(err, "Failed to load recipes");
      setPickerError(message);
    } finally {
      setRecipesLoading(false);
    }
  };

  const handleSearch = (e: Event) => {
    e.preventDefault();
    loadRecipes();
  };

  const addMealPlan = async (recipe: RecipeSummary) => {
    const date = pickerDate();
    const mealType = pickerMealType();
    if (!date || !mealType) return;

    try {
      await getMealPlansApi().createMealPlan({
        createMealPlanRequest: {
          recipeId: recipe.id,
          mealDate: toApiDate(date),
          mealType: mealType,
        },
      });
      setPickerOpen(false);
      await loadMealPlans();
    } catch (err) {
      const message = await extractApiError(err, "Failed to add meal");
      setError(message);
    }
  };

  const confirmDelete = (mp: MealPlanItem) => {
    setDeletingMealPlan(mp);
  };

  const handleDelete = async () => {
    const mp = deletingMealPlan();
    if (!mp) return;

    setDeleteLoading(true);
    try {
      await getMealPlansApi().deleteMealPlan({ id: mp.id });
      setDeletingMealPlan(null);
      await loadMealPlans();
    } catch (err) {
      const message = await extractApiError(err, "Failed to delete meal");
      setError(message);
      setDeletingMealPlan(null);
    } finally {
      setDeleteLoading(false);
    }
  };

  const isToday = (d: Date) => {
    const today = new Date();
    return (
      d.getDate() === today.getDate() &&
      d.getMonth() === today.getMonth() &&
      d.getFullYear() === today.getFullYear()
    );
  };

  return (
    <div class="meal-plan-page">
      <div class="page-header">
        <h2>Meal Plan</h2>
        <div class="week-nav">
          <button class="btn btn-small" onClick={prevWeek}>
            &larr; Prev
          </button>
          <button class="btn btn-small" onClick={goToToday}>
            Today
          </button>
          <button class="btn btn-small" onClick={nextWeek}>
            Next &rarr;
          </button>
        </div>
      </div>

      <Show when={error()}>
        <div class="error-message">{error()}</div>
      </Show>

      <Show when={loading()}>
        <p class="loading-text">Loading meal plans...</p>
      </Show>

      <Show when={!loading()}>
        <div class="meal-plan-grid">
          {/* Header row with day names */}
          <div class="meal-plan-header">
            <div class="meal-type-label"></div>
            <For each={weekDays()}>
              {(day) => (
                <div
                  class="day-header"
                  classList={{ "day-today": isToday(day) }}
                >
                  {formatDayHeader(day)}
                </div>
              )}
            </For>
          </div>

          {/* Meal type rows */}
          <For each={MEAL_TYPES}>
            {(mealType) => (
              <div class="meal-row">
                <div class="meal-type-label">{MEAL_TYPE_LABELS[mealType]}</div>
                <For each={weekDays()}>
                  {(day) => {
                    const meals = () => getMealsForSlot(day, mealType);
                    return (
                      <div
                        class="meal-slot"
                        classList={{ "day-today": isToday(day) }}
                      >
                        <For each={meals()}>
                          {(mp) => (
                            <div class="meal-card">
                              <Show when={mp.thumbnailPhotoId}>
                                <PhotoThumbnail
                                  photoId={mp.thumbnailPhotoId!}
                                  token={token()!}
                                  alt={mp.recipeTitle}
                                  class="meal-thumbnail"
                                />
                              </Show>
                              <A
                                href={`/recipes/${mp.recipeId}`}
                                class="meal-title"
                              >
                                {mp.recipeTitle}
                              </A>
                              <button
                                class="meal-remove"
                                onClick={() => confirmDelete(mp)}
                                title="Remove"
                              >
                                &times;
                              </button>
                            </div>
                          )}
                        </For>
                        <button
                          class="add-meal-btn"
                          onClick={() => openPicker(day, mealType)}
                          title="Add recipe"
                        >
                          +
                        </button>
                      </div>
                    );
                  }}
                </For>
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* Recipe picker modal */}
      <Modal
        isOpen={pickerOpen}
        onClose={() => setPickerOpen(false)}
        title={`Add ${pickerMealType() ? MEAL_TYPE_LABELS[pickerMealType()!] : ""} - ${pickerDate() ? formatDayHeader(pickerDate()!) : ""}`}
      >
        <form onSubmit={handleSearch} class="recipe-search-form">
          <input
            type="text"
            placeholder="Search recipes..."
            value={searchQuery()}
            onInput={(e) => setSearchQuery(e.currentTarget.value)}
            class="recipe-search-input"
          />
          <button type="submit" class="btn btn-small">
            Search
          </button>
        </form>

        <Show when={recipesLoading()}>
          <p class="loading-text">Loading recipes...</p>
        </Show>

        <Show when={pickerError()}>
          <p class="error">{pickerError()}</p>
        </Show>

        <Show
          when={!recipesLoading() && !pickerError() && recipes().length === 0}
        >
          <p class="empty-state">No recipes found.</p>
        </Show>

        <div class="recipe-picker-list">
          <For each={recipes()}>
            {(recipe) => (
              <div
                class="recipe-picker-item"
                onClick={() => addMealPlan(recipe)}
              >
                <Show when={recipe.thumbnailPhotoId}>
                  <PhotoThumbnail
                    photoId={recipe.thumbnailPhotoId!}
                    token={token()!}
                    alt={recipe.title}
                    class="recipe-picker-thumbnail"
                  />
                </Show>
                <span class="recipe-picker-title">{recipe.title}</span>
              </div>
            )}
          </For>
        </div>
      </Modal>

      {/* Delete confirmation modal */}
      <Modal
        isOpen={() => deletingMealPlan() !== null}
        onClose={() => setDeletingMealPlan(null)}
        title="Remove Meal"
        actions={
          <>
            <button
              class="btn"
              onClick={() => setDeletingMealPlan(null)}
              disabled={deleteLoading()}
            >
              Cancel
            </button>
            <button
              class="btn btn-danger"
              onClick={handleDelete}
              disabled={deleteLoading()}
            >
              {deleteLoading() ? "Removing..." : "Remove"}
            </button>
          </>
        }
      >
        <p>Remove "{deletingMealPlan()?.recipeTitle}" from your meal plan?</p>
      </Modal>
    </div>
  );
}
