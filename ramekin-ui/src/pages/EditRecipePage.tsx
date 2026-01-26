import { createSignal, Show, onMount } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { useParams, useNavigate, A } from "@solidjs/router";
import { useAuth } from "../context/AuthContext";
import RecipeForm from "../components/RecipeForm";
import { extractApiError } from "../utils/recipeFormHelpers";
import type { Ingredient, RecipeResponse } from "ramekin-client";

export default function EditRecipePage() {
  const params = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { getRecipesApi, getPhotosApi, token } = useAuth();

  const [loading, setLoading] = createSignal(true);
  const [title, setTitle] = createSignal("");
  const [description, setDescription] = createSignal("");
  const [photoIds, setPhotoIds] = createSignal<string[]>([]);
  const [uploading, setUploading] = createSignal(false);
  const [instructions, setInstructions] = createSignal("");
  const [sourceUrl, setSourceUrl] = createSignal("");
  const [sourceName, setSourceName] = createSignal("");
  const [tags, setTags] = createSignal<string[]>([]);
  const [ingredients, setIngredients] = createStore<Ingredient[]>([]);
  const [servings, setServings] = createSignal("");
  const [prepTime, setPrepTime] = createSignal("");
  const [cookTime, setCookTime] = createSignal("");
  const [totalTime, setTotalTime] = createSignal("");
  const [rating, setRating] = createSignal<number | null>(null);
  const [difficulty, setDifficulty] = createSignal("");
  const [nutritionalInfo, setNutritionalInfo] = createSignal("");
  const [notes, setNotes] = createSignal("");

  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const loadRecipe = async () => {
    setLoading(true);
    setError(null);
    try {
      const response: RecipeResponse = await getRecipesApi().getRecipe({
        id: params.id,
      });
      setTitle(response.title);
      setDescription(response.description || "");
      setInstructions(response.instructions);
      setSourceUrl(response.sourceUrl || "");
      setSourceName(response.sourceName || "");
      setTags(response.tags || []);
      setPhotoIds(response.photoIds || []);
      setIngredients(
        reconcile(
          response.ingredients?.length
            ? response.ingredients
            : [{ item: "", amount: "", unit: "" }],
        ),
      );
      setServings(response.servings || "");
      setPrepTime(response.prepTime || "");
      setCookTime(response.cookTime || "");
      setTotalTime(response.totalTime || "");
      setRating(response.rating ?? null);
      setDifficulty(response.difficulty || "");
      setNutritionalInfo(response.nutritionalInfo || "");
      setNotes(response.notes || "");
    } catch (err) {
      if (err instanceof Response && err.status === 404) {
        setError("Recipe not found");
      } else {
        setError("Failed to load recipe");
      }
    } finally {
      setLoading(false);
    }
  };

  onMount(() => {
    loadRecipe();
  });

  const handlePhotoUpload = async (e: Event) => {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    setUploading(true);
    setError(null);
    try {
      const response = await getPhotosApi().upload({ file });
      setPhotoIds([...photoIds(), response.id]);
    } catch (err) {
      const errorMessage = await extractApiError(err, "Failed to upload photo");
      setError(errorMessage);
    } finally {
      setUploading(false);
      input.value = "";
    }
  };

  const removePhoto = (photoId: string) => {
    setPhotoIds(photoIds().filter((id) => id !== photoId));
  };

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    setError(null);
    setSaving(true);

    try {
      const validIngredients = ingredients.filter(
        (ing) => ing.item.trim() !== "",
      );

      await getRecipesApi().updateRecipe({
        id: params.id,
        updateRecipeRequest: {
          title: title(),
          description: description() || undefined,
          instructions: instructions(),
          ingredients: validIngredients,
          sourceUrl: sourceUrl() || undefined,
          sourceName: sourceName() || undefined,
          tags: tags().length > 0 ? tags() : undefined,
          photoIds: photoIds(),
          servings: servings() || undefined,
          prepTime: prepTime() || undefined,
          cookTime: cookTime() || undefined,
          totalTime: totalTime() || undefined,
          rating: rating() ?? undefined,
          difficulty: difficulty() || undefined,
          nutritionalInfo: nutritionalInfo() || undefined,
          notes: notes() || undefined,
        },
      });

      navigate(`/recipes/${params.id}`);
    } catch (err) {
      const errorMessage = await extractApiError(
        err,
        "Failed to update recipe",
      );
      setError(errorMessage);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div class="edit-recipe-page">
      <h2>Edit Recipe</h2>

      <Show when={loading()}>
        <p class="loading">Loading recipe...</p>
      </Show>

      <Show when={error() && loading()}>
        <div class="error-state">
          <p class="error">{error()}</p>
          <A href="/" class="btn">
            Back to Cookbook
          </A>
        </div>
      </Show>

      <Show when={!loading()}>
        <RecipeForm
          title={title}
          setTitle={setTitle}
          description={description}
          setDescription={setDescription}
          instructions={instructions}
          setInstructions={setInstructions}
          sourceUrl={sourceUrl}
          setSourceUrl={setSourceUrl}
          sourceName={sourceName}
          setSourceName={setSourceName}
          tags={tags}
          setTags={setTags}
          servings={servings}
          setServings={setServings}
          prepTime={prepTime}
          setPrepTime={setPrepTime}
          cookTime={cookTime}
          setCookTime={setCookTime}
          totalTime={totalTime}
          setTotalTime={setTotalTime}
          rating={rating}
          setRating={setRating}
          difficulty={difficulty}
          setDifficulty={setDifficulty}
          nutritionalInfo={nutritionalInfo}
          setNutritionalInfo={setNutritionalInfo}
          notes={notes}
          setNotes={setNotes}
          ingredients={ingredients}
          setIngredients={setIngredients}
          photoIds={photoIds}
          onPhotoUpload={handlePhotoUpload}
          onPhotoRemove={removePhoto}
          uploading={uploading}
          saving={saving}
          error={error}
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
