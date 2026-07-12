import type {
  CreateRecipeRequest,
  Ingredient,
  RecipeResponse,
  UpdateRecipeRequest,
} from "ramekin-client";

export interface RecipeFormValues {
  title: string;
  description: string;
  instructions: string;
  sourceUrl: string;
  sourceName: string;
  tags: string[];
  photoIds: string[];
  ingredients: Ingredient[];
  servings: string;
  prepTime: string;
  cookTime: string;
  totalTime: string;
  rating: number | null;
  difficulty: string;
  nutritionalInfo: string;
  notes: string;
}

function emptyIngredient(): Ingredient {
  return { item: "", measurements: [{}] };
}

export function defaultRecipeFormValues(): RecipeFormValues {
  return {
    title: "",
    description: "",
    instructions: "",
    sourceUrl: "",
    sourceName: "",
    tags: [],
    photoIds: [],
    ingredients: [emptyIngredient()],
    servings: "",
    prepTime: "",
    cookTime: "",
    totalTime: "",
    rating: null,
    difficulty: "",
    nutritionalInfo: "",
    notes: "",
  };
}

export function emptyEditRecipeFormValues(): RecipeFormValues {
  return {
    ...defaultRecipeFormValues(),
    ingredients: [],
  };
}

export function recipeFormValuesFromRecipe(
  recipe: RecipeResponse,
): RecipeFormValues {
  return {
    title: recipe.title,
    description: recipe.description || "",
    instructions: recipe.instructions,
    sourceUrl: recipe.sourceUrl || "",
    sourceName: recipe.sourceName || "",
    tags: recipe.tags || [],
    photoIds: recipe.photoIds || [],
    ingredients: recipe.ingredients?.length
      ? recipe.ingredients
      : [emptyIngredient()],
    servings: recipe.servings || "",
    prepTime: recipe.prepTime || "",
    cookTime: recipe.cookTime || "",
    totalTime: recipe.totalTime || "",
    rating: recipe.rating ?? null,
    difficulty: recipe.difficulty || "",
    nutritionalInfo: recipe.nutritionalInfo || "",
    notes: recipe.notes || "",
  };
}

function validIngredients(ingredients: Ingredient[]): Ingredient[] {
  return ingredients.filter((ing) => ing.item.trim() !== "");
}

export function buildCreateRecipeRequest(
  values: RecipeFormValues,
): CreateRecipeRequest {
  return {
    title: values.title,
    description: values.description || undefined,
    instructions: values.instructions,
    ingredients: validIngredients(values.ingredients),
    sourceUrl: values.sourceUrl || undefined,
    sourceName: values.sourceName || undefined,
    tags: values.tags.length > 0 ? values.tags : undefined,
    photoIds: values.photoIds.length > 0 ? values.photoIds : undefined,
    servings: values.servings || undefined,
    prepTime: values.prepTime || undefined,
    cookTime: values.cookTime || undefined,
    totalTime: values.totalTime || undefined,
    rating: values.rating ?? undefined,
    difficulty: values.difficulty || undefined,
    nutritionalInfo: values.nutritionalInfo || undefined,
    notes: values.notes || undefined,
  };
}

export function buildUpdateRecipeRequest(
  values: RecipeFormValues,
  expectedVersionId: string,
): UpdateRecipeRequest {
  return {
    expectedVersionId,
    title: values.title,
    description: values.description || null,
    instructions: values.instructions,
    ingredients: validIngredients(values.ingredients),
    sourceUrl: values.sourceUrl || null,
    sourceName: values.sourceName || null,
    tags: values.tags.length > 0 ? values.tags : undefined,
    photoIds: values.photoIds,
    servings: values.servings || null,
    prepTime: values.prepTime || null,
    cookTime: values.cookTime || null,
    totalTime: values.totalTime || null,
    rating: values.rating ?? null,
    difficulty: values.difficulty || null,
    nutritionalInfo: values.nutritionalInfo || null,
    notes: values.notes || null,
  };
}
