import { describe, expect, it } from "vitest";
import type { RecipeResponse } from "ramekin-client";
import {
  buildCreateRecipeRequest,
  buildUpdateRecipeRequest,
  defaultRecipeFormValues,
  recipeFormValuesFromRecipe,
} from "./recipeFormSerialization";

describe("recipe form request serialization", () => {
  it("builds create requests with empty optional fields omitted", () => {
    const values = defaultRecipeFormValues();
    values.title = "Pancakes";
    values.instructions = "Mix and cook.";
    values.rating = 0;
    values.ingredients = [
      { item: "Flour", measurements: [{ amount: "1", unit: "cup" }] },
      { item: "   ", measurements: [{}] },
    ];

    expect(buildCreateRecipeRequest(values)).toEqual({
      title: "Pancakes",
      instructions: "Mix and cook.",
      ingredients: [
        { item: "Flour", measurements: [{ amount: "1", unit: "cup" }] },
      ],
      rating: 0,
    });
  });

  it("builds update requests with nullable cleared fields and photo ids", () => {
    const values = defaultRecipeFormValues();
    values.title = "Soup";
    values.instructions = "Simmer.";
    values.photoIds = [];

    expect(buildUpdateRecipeRequest(values, "version-1")).toEqual({
      expectedVersionId: "version-1",
      title: "Soup",
      description: null,
      instructions: "Simmer.",
      ingredients: [],
      sourceUrl: null,
      sourceName: null,
      tags: undefined,
      photoIds: [],
      servings: null,
      prepTime: null,
      cookTime: null,
      totalTime: null,
      rating: null,
      difficulty: null,
      nutritionalInfo: null,
      notes: null,
    });
  });

  it("loads recipe responses into editable form values", () => {
    const values = recipeFormValuesFromRecipe({
      id: "recipe-1",
      title: "Toast",
      description: null,
      instructions: "Toast bread.",
      ingredients: [],
      photoIds: ["photo-1"],
      tags: ["breakfast"],
      createdAt: new Date("2026-01-01T00:00:00Z"),
      updatedAt: new Date("2026-01-02T00:00:00Z"),
      versionId: "version-1",
      versionSource: "manual",
    } satisfies RecipeResponse);

    expect(values).toMatchObject({
      title: "Toast",
      description: "",
      instructions: "Toast bread.",
      photoIds: ["photo-1"],
      tags: ["breakfast"],
      ingredients: [{ item: "", measurements: [{}] }],
    });
  });
});
