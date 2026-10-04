import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createRoot } from "solid-js";
import type { PhotosApi, RecipeContent } from "ramekin-client";
import { createRecipeFormState } from "./recipeFormState";

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

  it("treats loaded and saved values as the new baseline", () => {
    withForm((form) => {
      const content: RecipeContent = {
        title: "Pancakes",
        instructions: "Mix.",
        ingredients: [{ item: "flour", measurements: [{ amount: "1" }] }],
      };
      form.loadDraft(content);
      expect(form.hasUnsavedChanges()).toBe(false);
      form.setNotes("Extra fluffy");
      expect(form.hasUnsavedChanges()).toBe(true);
      form.markSaved();
      expect(form.hasUnsavedChanges()).toBe(false);
    });
  });
});
