import { describe, expect, it } from "vitest";

import type { Ingredient } from "ramekin-client";
import vectorsJson from "../../../shared-test-vectors/ingredient-formatting.json?raw";

import {
  formatIngredient,
  formatIngredientAmount,
  formatIngredientParts,
} from "./ingredientFormatting";

function ingredient(overrides: Partial<Ingredient> & { item: string }) {
  return { measurements: [], ...overrides };
}

type IngredientFormattingVector = {
  name: string;
  ingredient: Ingredient;
  options: {
    scale?: number;
    includeAlternatives?: boolean;
    includeNote?: boolean;
  };
  expected: string;
};

const vectors = JSON.parse(vectorsJson) as IngredientFormattingVector[];

describe("formatIngredient", () => {
  it.each(vectors)("$name", ({ ingredient, options, expected }) => {
    expect(formatIngredient(ingredient, options)).toBe(expected);
  });
});

describe("formatIngredientParts", () => {
  it("separates the styled pieces without parens", () => {
    const ing = ingredient({
      item: "flour",
      measurements: [
        { amount: "1", unit: "cup" },
        { amount: "120", unit: "g" },
      ],
      note: "sifted",
    });

    expect(
      formatIngredientParts(ing, {
        scale: 2,
        includeAlternatives: true,
        includeNote: true,
      }),
    ).toEqual({
      amount: "2",
      unit: "cup",
      alternatives: "240 g",
      item: "flour",
      note: "sifted",
    });
  });

  it("omits alternatives and note unless requested", () => {
    const ing = ingredient({
      item: "flour",
      measurements: [
        { amount: "1", unit: "cup" },
        { amount: "120", unit: "g" },
      ],
      note: "sifted",
    });

    expect(formatIngredientParts(ing)).toEqual({
      amount: "1",
      unit: "cup",
      alternatives: null,
      item: "flour",
      note: null,
    });
  });
});

describe("formatIngredientAmount", () => {
  it("formats the scaled primary measurement", () => {
    const ing = ingredient({
      item: "flour",
      measurements: [{ amount: "1", unit: "cup" }],
    });

    expect(formatIngredientAmount(ing, 2)).toBe("2 cup");
  });

  it("returns undefined when there is nothing to show", () => {
    expect(formatIngredientAmount(ingredient({ item: "salt" }), 1)).toBe(
      undefined,
    );
    expect(
      formatIngredientAmount(
        ingredient({
          item: "salt",
          measurements: [{ amount: " ", unit: " " }],
        }),
        1,
      ),
    ).toBe(undefined);
  });

  it("formats a unit-less amount and an amount-less unit", () => {
    expect(
      formatIngredientAmount(
        ingredient({ item: "eggs", measurements: [{ amount: "2" }] }),
        1,
      ),
    ).toBe("2");
    expect(
      formatIngredientAmount(
        ingredient({ item: "salt", measurements: [{ unit: "pinch" }] }),
        1,
      ),
    ).toBe("pinch");
  });
});
