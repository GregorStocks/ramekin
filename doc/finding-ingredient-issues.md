# Finding Ingredient-Parsing Issues

This document explains how to find and document new ingredient parsing issues.

## Start from prod

The committed corpora (pipeline and Paprika fixtures, pipeline snapshots) are a
regression net, not the target: what matters is the owner's real recipes. Start
a parsing or catalog burndown from them.

```bash
make prod-export            # read-only copy of ~/code/ramekin/.db/dev.sqlite (PROD_DB=) into logs/prod/
make ingredient-prod-report # every failing prod line by cause: logs/prod/burndown.md and burndown.json
```

`make prod-export` copies the database with `sqlite3 -readonly .backup` and
exports the current version of every recipe that isn't deleted
(`logs/prod/recipes.json`) and the server's learned names
(`logs/prod/learned.json`). Those are also the `PROD_RECIPES=` and `LEARNED=`
inputs of `make ingredient-catalog-audit` and `make catalog-fill-weights`.

The report groups each line by the first thing that fails, in the order a fix
has to get through them:

- **Does not parse**: the stored amount can't be read, or several foods share one amount.
- **Name doesn't resolve**: no single catalog food (not recognized, or ambiguous).
- **Resolves but can't be weighed**: no weight for this unit or count, or no density for a volume.
- **Left unknown on purpose**: another recipe, frying oil.
- **No amount**: never counted, but never makes an estimate partial. Listed so misparsed amounts ("juice of half a lemon") show up.
- **No shopping category**: a food line the shopping list files under Other.

Prod stores each line as it was parsed when saved, with no raw text, so parser
fixes since then never reached older lines. The "fixed by re-parsing" column
writes each line out and parses it again with the current parser: those lines
need a re-normalize, not another parser rule. (The written-out line isn't the
original text, so treat it as approximate.)

Prod recipe text is the owner's data: everything stays in `logs/` (gitignored).
In PRs and issues, cite the report's group counts (`make ingredient-prod-report`
prints only counts) first and fixture counts second; quote prod lines only when
the owner has OKed them.

## Finding Issues via unique-ingredients.txt

The file `data/unique-ingredients.txt` is a sorted, deduplicated list of all ingredient names after parsing. This is useful for spotting patterns that suggest parsing problems (e.g., ingredient names that still contain quantities, prep notes, or junk). Regenerate it with `make pipeline`.

```bash
# Items that look like they still have quantities
rg -n "^[0-9]" data/unique-ingredients.txt

# Items with parentheses (may contain unparsed notes)
rg -n "\\(" data/unique-ingredients.txt

# Very long lines (often unparsed or malformed)
awk 'length > 50' data/unique-ingredients.txt
```

## Finding Issues via ingredient-categories.csv

The file `data/ingredient-categories.csv` is a quick way to spot pipeline parsing problems. Each row is a raw ingredient string paired with its assigned category; malformed or odd-looking strings often reveal parsing gaps or upstream extraction noise. Regenerate it with `make pipeline`.

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

When you find a recurring pattern, create an issue with a few concrete examples and (if possible) a rough count from `rg -c`.

## Creating New Issues

Create a new JSON5 issue file in `issues/` named `pN-short-summary.json5`, or `blocked-short-summary.json5` if it has an unresolved prerequisite. See `doc/issues.md` for the format.

Add the `ingredient-parser` label. If the issue is caused by upstream data problems (not fixable in the parser), also add `upstream`.
