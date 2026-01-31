#!/usr/bin/env python3
"""
Import ingredient densities from USDA FoodData Central SR Legacy.

Downloads SR Legacy data and extracts grams-per-cup for each ingredient,
then updates density_data.json.

Usage:
    uv run import_usda.py
"""

import csv
import json
import re
from pathlib import Path
from collections import defaultdict

# Conversion factors to cups
TBSP_PER_CUP = 16.0
TSP_PER_CUP = 48.0


def load_csv(path: Path) -> list[dict]:
    """Load a CSV file and return list of dicts."""
    with open(path, "r", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def parse_volume_unit(modifier: str) -> tuple[str | None, float]:
    """
    Parse volume unit from SR Legacy modifier field.
    Returns (unit_type, multiplier) where unit_type is 'cup', 'tbsp', 'tsp', or None.
    The multiplier accounts for partial measurements like "1/2 cup".
    """
    mod = modifier.lower().strip()

    # Skip entries with non-standard qualifiers
    # e.g., "cup chips" (not a typical measurement)
    if "chip" in mod:
        return None, 1.0

    # Check for cup - accept various forms like "cup", "cup, shredded", "cup, diced"
    if (
        mod == "cup"
        or mod.startswith("cup,")
        or mod.startswith("cup (")
        or mod == "cups"
    ):
        return "cup", 1.0
    if mod == "tbsp" or mod == "tablespoon":
        return "tbsp", 1.0
    if mod == "tsp" or mod == "teaspoon":
        return "tsp", 1.0

    return None, 1.0


def calculate_grams_per_cup(portions: list[dict]) -> float | None:
    """
    Calculate grams per cup from portion data.
    Prefers direct cup measurements, falls back to tbsp/tsp conversions.
    """
    cup_weights = []
    tbsp_weights = []
    tsp_weights = []

    for p in portions:
        modifier = p.get("modifier", "")
        unit_type, _ = parse_volume_unit(modifier)
        if unit_type is None:
            continue

        try:
            amount = float(p.get("amount", 0))
            gram_weight = float(p.get("gram_weight", 0))
        except (ValueError, TypeError):
            continue

        if amount <= 0 or gram_weight <= 0:
            continue

        # Calculate grams per single unit
        grams_per_unit = gram_weight / amount

        if unit_type == "cup":
            cup_weights.append(grams_per_unit)
        elif unit_type == "tbsp":
            # Convert tbsp to cup equivalent
            tbsp_weights.append(grams_per_unit * TBSP_PER_CUP)
        elif unit_type == "tsp":
            # Convert tsp to cup equivalent
            tsp_weights.append(grams_per_unit * TSP_PER_CUP)

    # Prefer cup measurements, then tbsp, then tsp
    if cup_weights:
        return sum(cup_weights) / len(cup_weights)
    elif tbsp_weights:
        return sum(tbsp_weights) / len(tbsp_weights)
    elif tsp_weights:
        return sum(tsp_weights) / len(tsp_weights)

    return None


def normalize_usda_name(description: str) -> str:
    """
    Normalize USDA description to a more recipe-friendly name.
    E.g., "Wheat flour, white, all-purpose, enriched, bleached"
    becomes "wheat flour, white, all-purpose".
    """
    name = description.lower().strip()

    # Remove common USDA suffixes that aren't useful for matching
    suffixes_to_remove = [
        ", enriched, bleached",
        ", enriched, unbleached",
        ", enriched",
        ", unenriched",
        ", raw",
        ", dry",
        " (includes foods for usda's food distribution program)",
    ]
    for suffix in suffixes_to_remove:
        if name.endswith(suffix):
            name = name[: -len(suffix)]

    return name


def extract_simple_name(description: str) -> str | None:
    """
    Extract a simple ingredient name from USDA description.
    E.g., "Butter, salted" -> "butter"
    Returns the simple name or None if not applicable.
    """
    name = description.lower().strip()

    # For entries like "Butter, salted" or "Butter, without salt", return just "butter"
    # But don't simplify "Flour, wheat" since "flour" alone is ambiguous
    simple_patterns = [
        (r"^butter,.*$", "butter"),
        (r"^milk,.*whole.*$", "milk"),
        (r"^milk,.*$", None),  # Don't alias other milks to just "milk"
        (r"^cream,.*heavy.*$", "heavy cream"),
        (r"^cream,.*sour.*$", "sour cream"),
        (r"^oil,.*olive.*$", "olive oil"),
        (r"^oil,.*vegetable.*$", "vegetable oil"),
        (r"^oil,.*coconut.*$", "coconut oil"),
        (r"^sugars,.*granulated.*$", "granulated sugar"),
        (r"^honey$", "honey"),
    ]

    for pattern, simple in simple_patterns:
        if re.match(pattern, name):
            return simple

    return None


def get_curated_ingredients() -> dict[str, float]:
    """
    Return curated ingredient densities for common baking ingredients.
    These override USDA data when there's a conflict.
    Sources: King Arthur Baking weight chart, various baking references.
    """
    return {
        # Flours (grams per cup)
        "all-purpose flour": 125.0,
        "bread flour": 127.0,
        "cake flour": 114.0,
        "whole wheat flour": 120.0,
        "almond flour": 96.0,
        "coconut flour": 112.0,
        # Sugars
        "granulated sugar": 200.0,
        "brown sugar": 220.0,  # packed
        "powdered sugar": 120.0,
        "honey": 340.0,
        "maple syrup": 315.0,
        # Dairy
        "butter": 227.0,
        "milk": 245.0,
        "heavy cream": 238.0,
        "sour cream": 242.0,
        "cream cheese": 232.0,
        # Fats/Oils
        "vegetable oil": 218.0,
        "olive oil": 216.0,
        "coconut oil": 218.0,
        # Other common
        "rolled oats": 80.0,
        "cornstarch": 128.0,
        "cocoa powder": 86.0,
        "peanut butter": 258.0,
    }


def get_curated_aliases() -> dict[str, str]:
    """Return curated aliases for common ingredient names."""
    return {
        # Flour aliases
        "flour": "all-purpose flour",
        "ap flour": "all-purpose flour",
        "plain flour": "all-purpose flour",
        "white flour": "all-purpose flour",
        # Sugar aliases
        "sugar": "granulated sugar",
        "white sugar": "granulated sugar",
        "caster sugar": "granulated sugar",
        "confectioners sugar": "powdered sugar",
        "confectioners' sugar": "powdered sugar",
        "icing sugar": "powdered sugar",
        "light brown sugar": "brown sugar",
        "dark brown sugar": "brown sugar",
        "packed brown sugar": "brown sugar",
        # Butter aliases
        "unsalted butter": "butter",
        "salted butter": "butter",
        # Oil aliases
        "oil": "vegetable oil",
        "canola oil": "vegetable oil",
        "extra virgin olive oil": "olive oil",
        "extra-virgin olive oil": "olive oil",
        # Cream aliases
        "whipping cream": "heavy cream",
        "heavy whipping cream": "heavy cream",
        "double cream": "heavy cream",
        "whole milk": "milk",
        # Oats aliases
        "oats": "rolled oats",
        "old-fashioned oats": "rolled oats",
        "old fashioned oats": "rolled oats",
        # Other aliases
        "corn starch": "cornstarch",
        "unsweetened cocoa powder": "cocoa powder",
        "dutch process cocoa powder": "cocoa powder",
        "natural cocoa powder": "cocoa powder",
        "pure maple syrup": "maple syrup",
    }


def main():
    script_dir = Path(__file__).parent
    data_dir = script_dir / "FoodData_Central_sr_legacy_food_csv_2018-04"

    if not data_dir.exists():
        print(f"Error: Data directory not found: {data_dir}")
        print("Please download SR Legacy data from USDA FoodData Central")
        return

    # Start with curated ingredients
    print("Loading curated ingredients...")
    ingredients = get_curated_ingredients()
    print(f"  {len(ingredients)} curated ingredients")

    print("\nLoading USDA SR Legacy data...")

    # Load food descriptions
    foods = load_csv(data_dir / "food.csv")
    food_map = {f["fdc_id"]: f["description"] for f in foods}
    print(f"  Loaded {len(food_map)} foods")

    # Load portions
    portions = load_csv(data_dir / "food_portion.csv")
    print(f"  Loaded {len(portions)} portion records")

    # Group portions by food
    portions_by_food = defaultdict(list)
    for p in portions:
        portions_by_food[p["fdc_id"]].append(p)

    # Calculate grams per cup for each food from USDA
    print("\nExtracting USDA densities...")
    usda_count = 0
    skipped_no_volume = 0

    aliases = get_curated_aliases()
    generated_aliases = {}

    for fdc_id, food_portions in portions_by_food.items():
        description = food_map.get(fdc_id, "")
        if not description:
            continue

        # Calculate grams per cup
        grams_per_cup = calculate_grams_per_cup(food_portions)
        if grams_per_cup is None:
            skipped_no_volume += 1
            continue

        # Normalize the USDA name
        normalized_name = normalize_usda_name(description)

        # Add to ingredients (don't overwrite curated data)
        if normalized_name not in ingredients:
            ingredients[normalized_name] = grams_per_cup
            usda_count += 1

            # Check if we can create a simple alias
            simple_name = extract_simple_name(description)
            if simple_name and simple_name in ingredients:
                # Add alias from USDA name to simple name
                if normalized_name not in aliases:
                    generated_aliases[normalized_name] = simple_name

    print(f"  Added {usda_count} ingredients from USDA")
    print(f"  Skipped {skipped_no_volume} foods without volume measurements")
    print(f"  Generated {len(generated_aliases)} aliases")

    # Merge generated aliases into curated aliases
    all_aliases = {**aliases, **generated_aliases}

    # Build JSON structure
    data = {
        "ingredients": dict(sorted(ingredients.items())),
        "aliases": dict(sorted(all_aliases.items())),
    }

    # Write JSON
    print("\nWriting JSON...")
    output_path = (
        script_dir.parent.parent / "ramekin-core" / "src" / "density_data.json"
    )
    with open(output_path, "w") as f:
        json.dump(data, f, indent=2)
    print(f"  Written to {output_path}")

    # Print some stats
    print("\nDensity data summary:")
    print(f"  Total ingredients: {len(ingredients)}")
    print(f"  Total aliases: {len(all_aliases)}")

    # Show some examples
    print("\nSample entries:")
    for name, grams in sorted(ingredients.items())[:15]:
        print(f"  {name}: {grams:.1f} g/cup")


if __name__ == "__main__":
    main()
