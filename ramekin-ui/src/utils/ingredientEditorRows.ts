import type { Ingredient } from "ramekin-client";

/**
 * The recipe form edits ingredients as one flat list of rows, where a section
 * row starts a named section that runs until the next section row. Keeping
 * sections as their own rows lets a section exist while it is empty, lets an
 * ingredient move between sections with a single reorder, and gives every row
 * a stable id so renaming a section never re-creates the rows below it.
 *
 * Behavior is pinned by shared-test-vectors/ingredient-editor-rows.json.
 */
export type IngredientEditorRow =
  | { kind: "section"; id: string; name: string }
  | { kind: "ingredient"; id: string; ingredient: Ingredient };

let nextRowId = 0;

function newRowId(): string {
  nextRowId += 1;
  return `ingredient-row-${nextRowId}`;
}

/** A section name with surrounding whitespace removed; blank means none. */
function normalizedSectionName(
  name: string | null | undefined,
): string | undefined {
  return name?.trim() || undefined;
}

export function sectionRow(name: string): IngredientEditorRow {
  return { kind: "section", id: newRowId(), name };
}

export function ingredientRow(ingredient: Ingredient): IngredientEditorRow {
  const withoutSection = { ...ingredient };
  delete withoutSection.section;
  return { kind: "ingredient", id: newRowId(), ingredient: withoutSection };
}

export function emptyIngredientRow(): IngredientEditorRow {
  return ingredientRow({ item: "", measurements: [{}] });
}

/**
 * Build editor rows, inserting a section row wherever the section changes. A
 * return to no section after a named one gets a blank-named section row so
 * those ingredients don't fall into the section above.
 */
export function rowsFromIngredients(
  ingredients: Ingredient[],
): IngredientEditorRow[] {
  const rows: IngredientEditorRow[] = [];
  let currentSection: string | undefined;
  for (const ingredient of ingredients) {
    const section = normalizedSectionName(ingredient.section);
    if (section !== currentSection) {
      currentSection = section;
      rows.push(sectionRow(section ?? ""));
    }
    rows.push(ingredientRow(ingredient));
  }
  return rows;
}

/**
 * Flatten editor rows back into ingredients. Each ingredient takes the trimmed
 * name of the nearest section row above it; a blank name means no section.
 */
export function ingredientsFromRows(rows: IngredientEditorRow[]): Ingredient[] {
  const ingredients: Ingredient[] = [];
  let currentSection: string | undefined;
  for (const row of rows) {
    if (row.kind === "section") {
      currentSection = normalizedSectionName(row.name);
    } else if (currentSection === undefined) {
      ingredients.push({ ...row.ingredient });
    } else {
      ingredients.push({ ...row.ingredient, section: currentSection });
    }
  }
  return ingredients;
}

/** Index just past the last row of the section whose heading is at `index`. */
export function sectionEndIndex(
  rows: IngredientEditorRow[],
  sectionIndex: number,
): number {
  let end = sectionIndex + 1;
  while (end < rows.length && rows[end].kind !== "section") {
    end += 1;
  }
  return end;
}
