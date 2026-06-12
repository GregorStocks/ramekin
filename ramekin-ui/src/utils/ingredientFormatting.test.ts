// Mirrors the formatting cases in
// ramekin-ios/RamekinTests/RecipeVersionSupportTests.swift, per
// doc/client-logic-sharing.md. Keep representative cases in sync until the
// shared-test-vector harness lands.
import { describe, expect, it } from "vitest";

import type { Ingredient } from "ramekin-client";

import {
  formatIngredient,
  formatIngredientAmount,
  formatIngredientParts,
} from "./ingredientFormatting";

function ingredient(overrides: Partial<Ingredient> & { item: string }) {
  return { measurements: [], ...overrides };
}

describe("formatIngredient", () => {
  it("defaults to primary measurement only", () => {
    const ing = ingredient({
      item: "flour",
      measurements: [
        { amount: "1", unit: "cup" },
        { amount: "120", unit: "g" },
      ],
      note: "sifted",
    });

    expect(formatIngredient(ing)).toBe("1 cup flour");
  });

  it("can include alternatives and note", () => {
    const ing = ingredient({
      item: "flour",
      measurements: [
        { amount: "1", unit: "cup" },
        { amount: "120", unit: "g" },
        { amount: "4.25", unit: "oz" },
      ],
      note: " sifted ",
    });

    expect(
      formatIngredient(ing, { includeAlternatives: true, includeNote: true }),
    ).toBe("1 cup (120 g, 4.25 oz) flour (sifted)");
  });

  it("skips blank measurements and blank note", () => {
    const ing = ingredient({
      item: "salt",
      measurements: [
        { amount: " ", unit: null },
        { amount: null, unit: " " },
        { amount: "1", unit: "tsp" },
      ],
      note: "   ",
    });

    expect(
      formatIngredient(ing, { includeAlternatives: true, includeNote: true }),
    ).toBe("(1 tsp) salt");
  });

  it("returns just the item when there are no measurements", () => {
    expect(formatIngredient(ingredient({ item: "salt" }))).toBe("salt");
  });

  it("scales the primary and alternative amounts", () => {
    const ing = ingredient({
      item: "flour",
      measurements: [
        { amount: "1", unit: "cup" },
        { amount: "1/2", unit: "stick" },
      ],
    });

    expect(formatIngredient(ing, { scale: 2, includeAlternatives: true })).toBe(
      "2 cup (1 stick) flour",
    );
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
