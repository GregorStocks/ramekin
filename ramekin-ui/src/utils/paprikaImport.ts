import { unzipSync, gunzipSync, strFromU8 } from "fflate";
import type { ImportRawRecipe } from "ramekin-client";

interface PaprikaPhoto {
  data?: string;
}

interface PaprikaRecipe {
  name: string;
  ingredients?: string;
  directions?: string;
  description?: string;
  notes?: string;
  source?: string;
  source_url?: string;
  categories?: string[];
  photos?: PaprikaPhoto[];
  photo_data?: string;
  servings?: string;
  prep_time?: string;
  cook_time?: string;
  total_time?: string;
  rating?: number;
  difficulty?: string;
  nutritional_info?: string;
}

export interface ParsedPaprikaRecipe {
  name: string;
  rawRecipe: ImportRawRecipe;
  photos: Uint8Array[];
}

export function parsePaprikaArchive(data: Uint8Array): ParsedPaprikaRecipe[] {
  const entries = unzipSync(data);
  const recipes: ParsedPaprikaRecipe[] = [];

  for (const [name, bytes] of Object.entries(entries)) {
    if (!name.endsWith(".paprikarecipe")) continue;

    const json = strFromU8(gunzipSync(bytes));
    const recipe = JSON.parse(json) as PaprikaRecipe;
    recipes.push(convertRecipe(recipe));
  }

  return recipes;
}

function convertRecipe(recipe: PaprikaRecipe): ParsedPaprikaRecipe {
  const photos: Uint8Array[] = [];
  if (recipe.photos && recipe.photos.length > 0) {
    for (const photo of recipe.photos) {
      if (photo.data) {
        photos.push(decodeBase64(photo.data));
      }
    }
  } else if (recipe.photo_data) {
    photos.push(decodeBase64(recipe.photo_data));
  }

  const rawRecipe: ImportRawRecipe = {
    title: recipe.name,
    description: recipe.description ?? null,
    ingredients: recipe.ingredients ?? "",
    instructions: recipe.directions ?? "",
    imageUrls: [],
    sourceUrl: recipe.source_url ?? null,
    sourceName: recipe.source ?? null,
    servings: recipe.servings ?? null,
    prepTime: recipe.prep_time ?? null,
    cookTime: recipe.cook_time ?? null,
    totalTime: recipe.total_time ?? null,
    rating: recipe.rating ?? null,
    difficulty: recipe.difficulty ?? null,
    nutritionalInfo: recipe.nutritional_info ?? null,
    notes: recipe.notes ?? null,
    categories: recipe.categories ?? null,
  };

  return { name: recipe.name, rawRecipe, photos };
}

function decodeBase64(s: string): Uint8Array {
  const binary = atob(s);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}
