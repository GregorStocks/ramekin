// Mirrors testFormatIngredientsIncludesAlternateMeasurementsNotesAndSections
// in ramekin-ios/RamekinTests/RecipeVersionSupportTests.swift, per
// doc/client-logic-sharing.md. Keep representative cases in sync until the
// shared-test-vector harness lands.
import { describe, expect, it } from "vitest";

import { formatIngredients, formatTags } from "./formatRecipeForDiff";

describe("formatIngredients", () => {
  it("includes alternate measurements, notes, and section headers", () => {
    const ingredients = [
      {
        item: "flour",
        measurements: [
          { amount: "1", unit: "cup" },
          { amount: "120", unit: "g" },
        ],
        note: "sifted",
        section: "Batter",
      },
      {
        item: "salt",
        measurements: [],
        section: "Batter",
      },
    ];

    expect(formatIngredients(ingredients)).toBe(
      "[Batter]\n1 cup (120 g) flour (sifted)\nsalt",
    );
  });

  it("emits a header at each section change but none for unsectioned ingredients", () => {
    const ingredients = [
      { item: "flour", measurements: [] },
      { item: "butter", measurements: [], section: "Topping" },
      { item: "sugar", measurements: [], section: "Topping" },
      { item: "salt", measurements: [] },
    ];

    expect(formatIngredients(ingredients)).toBe(
      "flour\n[Topping]\nbutter\nsugar\nsalt",
    );
  });
});

describe("formatTags", () => {
  it("joins tags and tolerates missing lists", () => {
    expect(formatTags(["dinner", "easy"])).toBe("dinner, easy");
    expect(formatTags(undefined)).toBe("");
    expect(formatTags(null)).toBe("");
  });
});
