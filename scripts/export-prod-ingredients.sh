#!/bin/bash
# Copy the prod database and export its live recipes' ingredients and the
# server's learned names for `make ingredient-prod-report` (and the
# PROD_RECIPES=/LEARNED= inputs of the other catalog targets).
#
# Reads prod only through `sqlite3 -readonly .backup`; every query runs on the
# copy. Everything lands in logs/prod/, which is gitignored: prod recipe text
# is the owner's data.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_DIR="$PROJECT_ROOT/logs/prod"
PROD_DB="${PROD_DB:-$HOME/code/ramekin/.db/dev.sqlite}"

if [ ! -f "$PROD_DB" ]; then
    echo "No prod database at $PROD_DB (set PROD_DB=)" >&2
    exit 1
fi

mkdir -p "$OUT_DIR"
COPY="$OUT_DIR/prod.sqlite"
rm -f "$COPY"
# VACUUM INTO fails on this database (its indexes use the server's fold_case).
sqlite3 -readonly "$PROD_DB" ".backup '$COPY'"

# Current versions of recipes that aren't deleted, in the shape the audit's
# --prod-recipes reads.
sqlite3 "$COPY" "
SELECT coalesce(json_group_array(json(recipe)), '[]')
FROM (
    SELECT json_object(
        'id', r.id,
        'title', v.title,
        'servings', v.servings,
        'ingredients', json(v.ingredients)
    ) AS recipe
    FROM recipes r
    JOIN recipe_versions v ON v.id = r.current_version_id
    WHERE r.deleted_at IS NULL
    ORDER BY r.seq
)
" >"$OUT_DIR/recipes.json"

sqlite3 "$COPY" "
SELECT coalesce(json_group_array(json(row)), '[]')
FROM (
    SELECT json_object(
        'name', name,
        'status', status,
        'disposition', disposition,
        'catalog_key', catalog_key,
        'kcal_per_100g', kcal_per_100g,
        'grams_per_cup', grams_per_cup,
        'grams_per_piece', grams_per_piece,
        'shopping_category', shopping_category
    ) AS row
    FROM ingredient_name_resolutions
    ORDER BY name
)
" >"$OUT_DIR/learned.json"

echo "Exported $(jq length "$OUT_DIR/recipes.json") recipes and $(jq length "$OUT_DIR/learned.json") learned names to logs/prod/"
