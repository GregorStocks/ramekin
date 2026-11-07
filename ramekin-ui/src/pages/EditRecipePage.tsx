import { createEffect, createSignal, Match, on, Switch } from "solid-js";
import { useParams, useNavigate, A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import RecipeForm from "../components/RecipeForm";
import {
  parseApiError,
  recipeUpdateErrorMessage,
} from "../utils/recipeFormHelpers";
import { createRecipeFormState } from "../utils/recipeFormState";
import { emptyEditRecipeFormValues } from "../utils/recipeFormSerialization";
import { ErrorCode } from "ramekin-client";
import { usePageTitle } from "../utils/pageTitle";
import { useUnsavedChangesGuard } from "../utils/unsavedChangesGuard";
import type { RecipeResponse } from "ramekin-client";

/** The form is only shown, and so only editable, once its recipe loads. */
type LoadState =
  | { kind: "loading" }
  | { kind: "failed"; message: string; retryable: boolean }
  | { kind: "loaded" };

export default function EditRecipePage() {
  const params = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { getRecipesApi, getPhotosApi, refreshTags, token } = useAuth();
  const [loadState, setLoadState] = createSignal<LoadState>({
    kind: "loading",
  });
  const loaded = () => loadState().kind === "loaded";
  const form = createRecipeFormState({
    getPhotosApi,
    initialValues: emptyEditRecipeFormValues(),
    pasteEnabled: loaded,
  });

  useUnsavedChangesGuard(() => loaded() && form.hasUnsavedChanges());

  usePageTitle(() => (form.title() ? `Edit: ${form.title()}` : "Edit Recipe"));

  // Bumped per load so a slow response for a recipe we've since left
  // can't overwrite the one now being edited.
  let loadGeneration = 0;

  const loadRecipe = async (id: string) => {
    const generation = ++loadGeneration;
    setLoadState({ kind: "loading" });
    form.setSaveError(null);
    try {
      const response: RecipeResponse = await getRecipesApi().getRecipe({ id });
      if (generation !== loadGeneration) return;
      form.loadRecipe(response);
      setLoadState({ kind: "loaded" });
    } catch (err) {
      if (generation !== loadGeneration) return;
      const parsed = await parseApiError(err, "Failed to load recipe");
      if (generation !== loadGeneration) return;
      const notFound = parsed.code === ErrorCode.NotFound;
      setLoadState({
        kind: "failed",
        message: notFound ? "Recipe not found" : "Failed to load recipe",
        retryable: !notFound,
      });
    }
  };

  createEffect(
    on(
      () => params.id,
      (id) => void loadRecipe(id),
    ),
  );

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    if (!loaded() || !form.canSubmit()) return;
    const id = params.id;
    form.setSaveError(null);
    form.setSaving(true);

    try {
      await getRecipesApi().updateRecipe({
        id,
        updateRecipeRequest: form.toUpdateRecipeRequest(),
      });

      // Refresh tags cache in case new tags were created
      refreshTags();

      form.markSaved();
      navigate(`/recipes/${id}`);
    } catch (err) {
      const parsed = await parseApiError(err, "Failed to update recipe");
      form.setSaveError(recipeUpdateErrorMessage(parsed));
    } finally {
      form.setSaving(false);
    }
  };

  return (
    <div class="edit-recipe-page">
      <h2>Edit Recipe</h2>

      <Switch>
        <Match when={loadState().kind === "loading"}>
          <p class="loading">Loading recipe...</p>
        </Match>
        <Match
          when={(() => {
            const state = loadState();
            return state.kind === "failed" ? state : null;
          })()}
        >
          {(failed) => (
            <div class="error-state">
              <p class="error">{failed().message}</p>
              <div class="error-state-actions">
                {failed().retryable && (
                  <button
                    type="button"
                    class="btn btn-primary"
                    onClick={() => void loadRecipe(params.id)}
                  >
                    Retry
                  </button>
                )}
                <A href="/" class="btn">
                  Back to Cookbook
                </A>
              </div>
            </div>
          )}
        </Match>
        <Match when={loaded()}>
          <RecipeForm
            form={form}
            onSubmit={handleSubmit}
            submitLabel="Save Changes"
            submitLabelSaving="Saving..."
            cancelHref={`/recipes/${params.id}`}
            token={token}
          />
        </Match>
      </Switch>
    </div>
  );
}
