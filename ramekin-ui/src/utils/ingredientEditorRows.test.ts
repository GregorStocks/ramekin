import { describe, expect, it } from "vitest";

import type { Ingredient } from "ramekin-client";
import vectorsJson from "../../../shared-test-vectors/ingredient-editor-rows.json?raw";

import {
  ingredientRow,
  ingredientsFromRows,
  rowsFromIngredients,
  sectionEndIndex,
  sectionRow,
} from "./ingredientEditorRows";
import type { IngredientEditorRow } from "./ingredientEditorRows";

type VectorRow = { section: string } | { ingredient: Ingredient };

type IngredientEditorRowsVectors = {
  fromIngredients: Array<{
    name: string;
    ingredients: Ingredient[];
    rows: VectorRow[];
  }>;
  toIngredients: Array<{
    name: string;
    rows: VectorRow[];
    ingredients: Ingredient[];
  }>;
};

const vectors = JSON.parse(vectorsJson) as IngredientEditorRowsVectors;

function toVectorRow(row: IngredientEditorRow): VectorRow {
  return row.kind === "section"
    ? { section: row.name }
    : { ingredient: row.ingredient };
}

function fromVectorRow(row: VectorRow): IngredientEditorRow {
  return "section" in row
    ? sectionRow(row.section)
    : ingredientRow(row.ingredient);
}

describe("rowsFromIngredients", () => {
  it.each(vectors.fromIngredients)("$name", ({ ingredients, rows }) => {
    expect(rowsFromIngredients(ingredients).map(toVectorRow)).toEqual(rows);
  });

  it("gives every row a distinct id", () => {
    const rows = rowsFromIngredients([
      { item: "flour", measurements: [], section: "Batter" },
      { item: "milk", measurements: [], section: "Batter" },
    ]);
    expect(new Set(rows.map((row) => row.id)).size).toBe(rows.length);
  });
});

describe("ingredientsFromRows", () => {
  it.each(vectors.toIngredients)("$name", ({ rows, ingredients }) => {
    expect(ingredientsFromRows(rows.map(fromVectorRow))).toEqual(ingredients);
  });

  it.each(vectors.fromIngredients)("round-trips $name", ({ ingredients }) => {
    const normalized = ingredients.map((ingredient) => {
      const section = ingredient.section?.trim();
      const copy = { ...ingredient };
      delete copy.section;
      return section ? { ...copy, section } : copy;
    });
    expect(ingredientsFromRows(rowsFromIngredients(ingredients))).toEqual(
      normalized,
    );
  });
});

describe("sectionEndIndex", () => {
  it("stops at the next section heading or the end of the list", () => {
    const rows = [
      sectionRow("Batter"),
      ingredientRow({ item: "flour", measurements: [] }),
      sectionRow("Glaze"),
      ingredientRow({ item: "sugar", measurements: [] }),
    ];
    expect(sectionEndIndex(rows, 0)).toBe(2);
    expect(sectionEndIndex(rows, 2)).toBe(4);
  });
});
