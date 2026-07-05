import { createSignal, Show, onMount } from "solid-js";
import { useParams, useNavigate, A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import RecipeForm from "../components/RecipeForm";
import { extractApiError, parseApiError } from "../utils/recipeFormHelpers";
import { createRecipeFormState } from "../utils/recipeFormState";
import { emptyEditRecipeFormValues } from "../utils/recipeFormSerialization";
import { ErrorCode } from "ramekin-client";
import { usePageTitle } from "../utils/pageTitle";
import type { RecipeResponse } from "ramekin-client";

export default function EditRecipePage() {
  const params = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { getRecipesApi, getPhotosApi, refreshTags, token } = useAuth();
  const [loading, setLoading] = createSignal(true);
  const form = createRecipeFormState({
    getPhotosApi,
    initialValues: emptyEditRecipeFormValues(),
    pasteEnabled: () => !loading(),
  });

  usePageTitle(() => (form.title() ? `Edit: ${form.title()}` : "Edit Recipe"));

  const loadRecipe = async () => {
    setLoading(true);
    form.setError(null);
    try {
      const response: RecipeResponse = await getRecipesApi().getRecipe({
        id: params.id,
      });
      form.loadRecipe(response);
    } catch (err) {
      const parsed = await parseApiError(err, "Failed to load recipe");
      form.setError(
        parsed.code === ErrorCode.NotFound
          ? "Recipe not found"
          : "Failed to load recipe",
      );
    } finally {
      setLoading(false);
    }
  };

  onMount(() => {
    loadRecipe();
  });

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    form.setError(null);
    form.setSaving(true);

    try {
      await getRecipesApi().updateRecipe({
        id: params.id,
        updateRecipeRequest: form.toUpdateRecipeRequest(),
      });

      // Refresh tags cache in case new tags were created
      refreshTags();

      navigate(`/recipes/${params.id}`);
    } catch (err) {
      const errorMessage = await extractApiError(
        err,
        "Failed to update recipe",
      );
      form.setError(errorMessage);
    } finally {
      form.setSaving(false);
    }
  };

  return (
    <div class="edit-recipe-page">
      <h2>Edit Recipe</h2>

      <Show when={loading()}>
        <p class="loading">Loading recipe...</p>
      </Show>

      <Show when={form.error() && loading()}>
        <div class="error-state">
          <p class="error">{form.error()}</p>
          <A href="/" class="btn">
            Back to Cookbook
          </A>
        </div>
      </Show>

      <Show when={!loading()}>
        <RecipeForm
          form={form}
          onSubmit={handleSubmit}
          submitLabel="Save Changes"
          submitLabelSaving="Saving..."
          cancelHref={`/recipes/${params.id}`}
          token={token}
        />
      </Show>
    </div>
  );
}
