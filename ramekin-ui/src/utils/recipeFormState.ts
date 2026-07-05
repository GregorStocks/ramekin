import { createSignal, onCleanup, onMount } from "solid-js";
import type { Accessor, Setter } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import type { SetStoreFunction } from "solid-js/store";
import type {
  CreateRecipeRequest,
  Ingredient,
  PhotosApi,
  RecipeResponse,
  UpdateRecipeRequest,
} from "ramekin-client";
import { extractApiError, extractImageFile } from "./recipeFormHelpers";
import {
  buildCreateRecipeRequest,
  buildUpdateRecipeRequest,
  defaultRecipeFormValues,
  recipeFormValuesFromRecipe,
} from "./recipeFormSerialization";
import type { RecipeFormValues } from "./recipeFormSerialization";

export interface RecipeFormState {
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
  photoIds: Accessor<string[]>;
  setPhotoIds: Setter<string[]>;
  ingredients: Ingredient[];
  setIngredients: SetStoreFunction<Ingredient[]>;
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
  uploading: Accessor<boolean>;
  saving: Accessor<boolean>;
  setSaving: Setter<boolean>;
  error: Accessor<string | null>;
  setError: Setter<string | null>;
  onPhotoUpload: (e: Event) => Promise<void>;
  removePhoto: (photoId: string) => void;
  loadRecipe: (recipe: RecipeResponse) => void;
  toCreateRecipeRequest: () => CreateRecipeRequest;
  toUpdateRecipeRequest: () => UpdateRecipeRequest;
}

interface CreateRecipeFormStateOptions {
  getPhotosApi: () => PhotosApi;
  initialValues?: RecipeFormValues;
  pasteEnabled?: Accessor<boolean>;
}

export function createRecipeFormState(
  options: CreateRecipeFormStateOptions,
): RecipeFormState {
  const initialValues = options.initialValues ?? defaultRecipeFormValues();
  const pasteEnabled = options.pasteEnabled ?? (() => true);

  const [title, setTitle] = createSignal(initialValues.title);
  const [description, setDescription] = createSignal(initialValues.description);
  const [instructions, setInstructions] = createSignal(
    initialValues.instructions,
  );
  const [sourceUrl, setSourceUrl] = createSignal(initialValues.sourceUrl);
  const [sourceName, setSourceName] = createSignal(initialValues.sourceName);
  const [tags, setTags] = createSignal<string[]>(initialValues.tags);
  const [photoIds, setPhotoIds] = createSignal<string[]>(
    initialValues.photoIds,
  );
  const [ingredients, setIngredients] = createStore<Ingredient[]>(
    initialValues.ingredients,
  );
  const [servings, setServings] = createSignal(initialValues.servings);
  const [prepTime, setPrepTime] = createSignal(initialValues.prepTime);
  const [cookTime, setCookTime] = createSignal(initialValues.cookTime);
  const [totalTime, setTotalTime] = createSignal(initialValues.totalTime);
  const [rating, setRating] = createSignal<number | null>(initialValues.rating);
  const [difficulty, setDifficulty] = createSignal(initialValues.difficulty);
  const [nutritionalInfo, setNutritionalInfo] = createSignal(
    initialValues.nutritionalInfo,
  );
  const [notes, setNotes] = createSignal(initialValues.notes);
  const [uploading, setUploading] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const values = (): RecipeFormValues => ({
    title: title(),
    description: description(),
    instructions: instructions(),
    sourceUrl: sourceUrl(),
    sourceName: sourceName(),
    tags: tags(),
    photoIds: photoIds(),
    ingredients,
    servings: servings(),
    prepTime: prepTime(),
    cookTime: cookTime(),
    totalTime: totalTime(),
    rating: rating(),
    difficulty: difficulty(),
    nutritionalInfo: nutritionalInfo(),
    notes: notes(),
  });

  const loadValues = (nextValues: RecipeFormValues) => {
    setTitle(nextValues.title);
    setDescription(nextValues.description);
    setInstructions(nextValues.instructions);
    setSourceUrl(nextValues.sourceUrl);
    setSourceName(nextValues.sourceName);
    setTags(nextValues.tags);
    setPhotoIds(nextValues.photoIds);
    setIngredients(reconcile(nextValues.ingredients));
    setServings(nextValues.servings);
    setPrepTime(nextValues.prepTime);
    setCookTime(nextValues.cookTime);
    setTotalTime(nextValues.totalTime);
    setRating(nextValues.rating);
    setDifficulty(nextValues.difficulty);
    setNutritionalInfo(nextValues.nutritionalInfo);
    setNotes(nextValues.notes);
  };

  const uploadPhotoFile = async (file: File) => {
    if (uploading()) return;
    setUploading(true);
    setError(null);
    try {
      const response = await options.getPhotosApi().upload({ file });
      setPhotoIds([...photoIds(), response.id]);
    } catch (err) {
      const errorMessage = await extractApiError(err, "Failed to upload photo");
      setError(errorMessage);
    } finally {
      setUploading(false);
    }
  };

  const onPhotoUpload = async (e: Event) => {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    try {
      await uploadPhotoFile(file);
    } finally {
      input.value = "";
    }
  };

  const handlePaste = (e: ClipboardEvent) => {
    if (!pasteEnabled()) return;
    const file = extractImageFile(e.clipboardData);
    if (!file) return;
    e.preventDefault();
    void uploadPhotoFile(file);
  };

  onMount(() => {
    document.addEventListener("paste", handlePaste);
  });

  onCleanup(() => {
    document.removeEventListener("paste", handlePaste);
  });

  return {
    title,
    setTitle,
    description,
    setDescription,
    instructions,
    setInstructions,
    sourceUrl,
    setSourceUrl,
    sourceName,
    setSourceName,
    tags,
    setTags,
    photoIds,
    setPhotoIds,
    ingredients,
    setIngredients,
    servings,
    setServings,
    prepTime,
    setPrepTime,
    cookTime,
    setCookTime,
    totalTime,
    setTotalTime,
    rating,
    setRating,
    difficulty,
    setDifficulty,
    nutritionalInfo,
    setNutritionalInfo,
    notes,
    setNotes,
    uploading,
    saving,
    setSaving,
    error,
    setError,
    onPhotoUpload,
    removePhoto: (photoId: string) => {
      setPhotoIds(photoIds().filter((id) => id !== photoId));
    },
    loadRecipe: (recipe: RecipeResponse) => {
      loadValues(recipeFormValuesFromRecipe(recipe));
    },
    toCreateRecipeRequest: () => buildCreateRecipeRequest(values()),
    toUpdateRecipeRequest: () => buildUpdateRecipeRequest(values()),
  };
}
