# ingredient-density

Ingredient density lookup for volume-to-weight conversion in recipes.

## Overview

This crate provides density data (grams per US cup) for common cooking
ingredients, enabling conversion from volume measurements (cups, tbsp, tsp)
to weight (grams).

## Data Sources

- **USDA FoodData Central SR Legacy** - Primary source, public domain (CC0)
- **Curated overrides** - Hand-verified values with citations for specific ingredients

## Usage

```rust
use ingredient_density::{find_density, volume_to_cups};

// Look up density for flour (returns grams per cup)
if let Some(grams_per_cup) = find_density("all-purpose flour") {
    // Convert 2 cups to grams
    let cups = volume_to_cups(2.0, "cup").unwrap();
    let grams = cups * grams_per_cup;
    println!("2 cups flour = {grams}g");  // 250g
}
```

## Lookup Behavior

`find_density()` tries to match ingredients in this order:

1. Direct lookup in ingredients database
2. Lookup via aliases
3. Plural/singular variations (e.g., "onion" → "onions")
4. After stripping temperature modifiers (e.g., "softened butter" → "butter")

Returns `None` for unknown ingredients or explicitly ambiguous terms.

## Adding Curated Data

Edit `src/data/curated.json` to add verified density values:

```json
{
  "ingredients": {
    "salt, diamond crystal kosher": {
      "grams_per_cup": 135,
      "source": "America's Test Kitchen",
      "url": "https://..."
    }
  },
  "aliases": {
    "table salt": "salt, table",
    "sea salt": null  // null = ambiguous, returns None
  }
}
```

All curated entries require a source citation.

## Regenerating USDA Data

Run `make ingredient-density-import` from the repository root. It uses `uv`
and the Python standard library to download the fixed April 2018 SR Legacy
CSV archive, verify its SHA-256, and write `src/data/usda.json`. No API key or
manual extraction is needed. The archive is cached at
`.cache/nutrition-sr-legacy-2018-04.zip`, shared with `make nutrition-import`.
Every run verifies the checksum, including cached runs. A checksum mismatch
fails; review the source before changing the pin.

The generated file records the source URL, release, checksum, and excluded
portion IDs. Repeated imports of this archive produce identical bytes.
Commit the regenerated file alongside changes to the importer.

The importer in `scripts/usda-import/import_usda.py` uses these selection rules:

- Divide portion gram weight by amount. Prefer cup portions, then tablespoons
  (16 per cup), then teaspoons (48 per cup). Average measurements within the
  preferred unit in archive row order.
- Accept `cup`, `cups`, `cup, ...`, and `cup (...)`; omit chip portions and
  unsupported units. Preparation qualifiers are not retained in density keys.
- Normalize descriptions to lowercase and strip the suffixes listed in
  `normalize_usda_name`. When normalized names collide, the first food with
  volume data in the pinned portion table wins.
- Start with the existing 23 manual baking densities and common-name aliases
  in `get_curated_ingredients` / `get_curated_aliases`; manual values take
  precedence. These values are not USDA measurements. Their original source
  attribution is only “King Arthur Baking weight chart, various baking
  references”; individual citations have not yet been established.
- Generate additional aliases using the explicit patterns in
  `extract_simple_name`. At runtime, `curated.json` overrides this generated
  dataset as described above.
- Exclude seven explicitly identified cup rows with zero amounts in this
  release. Their IDs and expected values are pinned in `EXCLUDED_PORTIONS`;
  all other malformed supported-volume measurements fail the import.

The importer also rejects empty tables, duplicate food/portion IDs, unknown
food references, invalid generated densities, and dangling aliases before
writing output. Its offline regression tests run under `make test`.

## License

MIT
