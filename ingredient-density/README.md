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

Returns `None` for unknown ingredients or explicitly ambiguous terms (like
"kosher salt" which could be Diamond Crystal or Morton, with very different
densities).

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
    "kosher salt": null  // null = ambiguous, returns None
  }
}
```

All curated entries require a source citation.

## Regenerating USDA Data

The USDA data is pre-generated and checked into the repo. To regenerate:

1. Download USDA FoodData Central SR Legacy CSV files
2. Run `cargo run --bin import_usda`

## License

MIT
