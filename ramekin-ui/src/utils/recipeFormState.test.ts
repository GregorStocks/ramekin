import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createRoot } from "solid-js";
import type { PhotosApi, RecipeResponse } from "ramekin-client";
import { createRecipeFormState } from "./recipeFormState";
import {
  buildUpdateRecipeRequest,
  defaultRecipeFormValues,
  recipeFormValuesFromRecipe,
} from "./recipeFormSerialization";

describe("recipe form request serialization", () => {
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
      derivedMeasurements: [],
      photoIds: ["photo-1"],
      tags: ["breakfast"],
      createdAt: new Date("2026-01-01T00:00:00Z"),
      updatedAt: new Date("2026-01-02T00:00:00Z"),
      versionId: "version-1",
      versionSource: "manual",
      normalizeOps: [],
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

describe("createRecipeFormState unsaved changes", () => {
  // The form registers a document paste listener on mount.
  const originalDocument = globalThis.document;
  beforeEach(() => {
    globalThis.document = {
      addEventListener: () => {},
      removeEventListener: () => {},
    } as unknown as Document;
  });
  afterEach(() => {
    globalThis.document = originalDocument;
  });

  const withForm = (
    run: (form: ReturnType<typeof createRecipeFormState>) => void,
  ) =>
    createRoot((dispose) => {
      run(createRecipeFormState({ getPhotosApi: () => ({}) as PhotosApi }));
      dispose();
    });

  it("is clean until a field changes, and clean again when reverted", () => {
    withForm((form) => {
      expect(form.hasUnsavedChanges()).toBe(false);
      form.setTitle("Soup");
      expect(form.hasUnsavedChanges()).toBe(true);
      form.setTitle("");
      expect(form.hasUnsavedChanges()).toBe(false);
    });
  });

  it("counts a photo upload in progress as unsaved", () => {
    createRoot((dispose) => {
      const form = createRecipeFormState({
        getPhotosApi: () =>
          ({ upload: () => new Promise(() => {}) }) as unknown as PhotosApi,
      });
      const input = { files: [new Blob(["x"])], value: "photo.jpg" };
      void form.onPhotoUpload({ target: input } as unknown as Event);
      expect(form.uploading()).toBe(true);
      expect(form.hasUnsavedChanges()).toBe(true);
      dispose();
    });
  });

  it("treats loaded and saved values as the new baseline", () => {
    withForm((form) => {
      form.loadRecipe({
        id: "recipe-1",
        title: "Pancakes",
        instructions: "Mix.",
        ingredients: [{ item: "flour", measurements: [{ amount: "1" }] }],
        derivedMeasurements: [],
        photoIds: [],
        tags: [],
        createdAt: new Date("2026-01-01T00:00:00Z"),
        updatedAt: new Date("2026-01-02T00:00:00Z"),
        versionId: "version-1",
        versionSource: "manual",
        normalizeOps: [],
      });
      expect(form.hasUnsavedChanges()).toBe(false);
      form.setNotes("Extra fluffy");
      expect(form.hasUnsavedChanges()).toBe(true);
      form.markSaved();
      expect(form.hasUnsavedChanges()).toBe(false);
    });
  });
});
