# So You've Been Told to Create New Ingredient-Parsing Beads

This document explains how to find and document new ingredient parsing issues.

## Finding New Issues via unique-ingredients.txt

The file `data/unique-ingredients.txt` is a sorted, deduplicated list of all ingredient names after parsing. This is useful for spotting patterns that suggest parsing problems (e.g., ingredient names that still contain quantities, prep notes, or junk). Regenerate it with `make pipeline`.

```bash
# Items that look like they still have quantities
rg -n "^[0-9]" data/unique-ingredients.txt

# Items with parentheses (may contain unparsed notes)
rg -n "\\(" data/unique-ingredients.txt

# Very long lines (often unparsed or malformed)
awk 'length > 50' data/unique-ingredients.txt
```

## Finding New Issues via ingredient-categories.csv

The file `data/ingredient-categories.csv` is a quick way to spot pipeline parsing problems. Each row is a raw ingredient string paired with its assigned category; malformed or odd-looking strings often reveal parsing gaps or upstream extraction noise. Regenerate it with `make pipeline` (runs the pipeline and refreshes the CSV) or `make ingredient-categories-generate`.

Use targeted searches to find patterns:

```bash
# Leading bullets / list markers that shouldn't be part of the ingredient
rg -n "^[&+\\-*]" data/ingredient-categories.csv

# Lines starting with a parenthetical (often hidden quantities)
rg -n "^\\(" data/ingredient-categories.csv

# Orphaned continuation lines (likely belong to previous ingredient)
rg -n "^(and|or|plus) " data/ingredient-categories.csv

# Section headers or equipment lines (frequently end with colons or "pan", "bowl", etc.)
rg -n ":[^,]*,|\\bpan\\b|\\bbowl\\b|\\bskillet\\b" data/ingredient-categories.csv
```

When you find a recurring pattern, add a bead with a few concrete examples and (if possible) a rough count from `rg -c`.

## Creating New Beads

When you create a new ingredient-parsing bead, include a reference to the documentation:

```
See doc/ingredient-parsing.md for workflow and file locations.
```

Add the `ingredient-parser` label to make it discoverable via `bd list -l ingredient-parser`.
