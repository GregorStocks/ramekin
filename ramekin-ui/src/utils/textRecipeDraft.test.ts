import { describe, expect, it } from "vitest";
import {
  CreateRecipeRequestToJSON,
  RecipeContentFromJSON,
} from "ramekin-client";
import vectors from "../../../shared-test-vectors/text-recipe-draft.json";
import {
  buildCreateRecipeRequest,
  recipeFormValuesFromRecipe,
} from "./recipeFormSerialization";

describe("text draft review preserves content (shared with iOS)", () => {
  for (const [index, vector] of vectors.entries()) {
    it(`round trips draft ${index}`, () => {
      const values = recipeFormValuesFromRecipe(RecipeContentFromJSON(vector));
      const request = buildCreateRecipeRequest(values);
      expect(
        JSON.parse(JSON.stringify(CreateRecipeRequestToJSON(request))),
      ).toEqual(vector);
    });
  }
});
