#!/usr/bin/env python3
"""
Import ingredient densities from USDA FoodData Central SR Legacy.

Downloads SR Legacy data and extracts grams-per-cup for each ingredient,
then updates ingredient-density/src/data/usda.json.

Usage:
    make ingredient-density-import
"""

import csv
import hashlib
import io
import json
import math
import re
import urllib.request
import zipfile
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = "https://fdc.nal.usda.gov/fdc-datasets/FoodData_Central_sr_legacy_food_csv_2018-04.zip"
ARCHIVE_SHA256 = "b80817294b8850530aaedf2e515c02593b1824f763a0ff356e5c2081643e6fd0"
CACHE = ROOT / ".cache/nutrition-sr-legacy-2018-04.zip"
OUTPUT = ROOT / "ingredient-density/src/data/usda.json"
# These pinned-source cup rows have zero amounts and cannot define a density.
# Keep the exact identifying values so a source correction requires review.
EXCLUDED_PORTIONS = {
    "83785": ("168789", "cup", "0", "142"),
    "83786": ("168790", "cup", "0", "142"),
    "83793": ("168796", "cup", "0", "141"),
    "85231": ("169617", "cup", "0", "142"),
    "85240": ("169621", "cup", "0", "141"),
    "90178": ("172252", "cup", "0", "7"),
    "92745": ("173509", "cup", "0", "16"),
}

# Conversion factors to cups
TBSP_PER_CUP = 16.0
TSP_PER_CUP = 48.0


def read_rows(archive: zipfile.ZipFile, filename: str) -> list[dict[str, str]]:
    """Read exactly one CSV member without extracting archive paths."""
    matches = [name for name in archive.namelist() if name.endswith("/" + filename)]
    if len(matches) != 1:
        raise ValueError(f"Expected exactly one {filename}: {matches}")
    with archive.open(matches[0]) as stream:
        return list(csv.DictReader(io.TextIOWrapper(stream, encoding="utf-8-sig")))


def verify_archive(contents: bytes) -> None:
    if hashlib.sha256(contents).hexdigest() != ARCHIVE_SHA256:
        raise ValueError(
            "USDA archive checksum mismatch; review source before updating"
        )


def load_archive(cache: Path) -> bytes:
    if cache.exists():
        contents = cache.read_bytes()
        verify_archive(contents)
        return contents
    with urllib.request.urlopen(SOURCE, timeout=120) as response:
        contents = response.read()
    verify_archive(contents)
    cache.parent.mkdir(parents=True, exist_ok=True)
    cache.write_bytes(contents)
    return contents


def parse_volume_unit(modifier: str) -> str | None:
    """
    Parse volume unit from SR Legacy modifier field.
    Returns 'cup', 'tbsp', 'tsp', or None. Fractions are in the amount field.
    """
    mod = modifier.lower().strip()

    # Skip entries with non-standard qualifiers
    # e.g., "cup chips" (not a typical measurement)
    if "chip" in mod:
        return None

    # Check for cup - accept various forms like "cup", "cup, shredded", "cup, diced"
    if (
        mod == "cup"
        or mod.startswith("cup,")
        or mod.startswith("cup (")
        or mod == "cups"
    ):
        return "cup"
    if mod == "tbsp" or mod == "tablespoon":
        return "tbsp"
    if mod == "tsp" or mod == "teaspoon":
        return "tsp"

    return None


def calculate_grams_per_cup(portions: list[dict]) -> float | None:
    """
    Calculate grams per cup from portion data.
    Prefers direct cup measurements, falls back to tbsp/tsp conversions.
    """
    cup_weights = []
    tbsp_weights = []
    tsp_weights = []

    for p in portions:
        if p["id"] in EXCLUDED_PORTIONS:
            values = tuple(
                p[key] for key in ("fdc_id", "modifier", "amount", "gram_weight")
            )
            if values != EXCLUDED_PORTIONS[p["id"]]:
                raise ValueError(f"Excluded USDA portion changed: {p}")
            continue
        unit_type = parse_volume_unit(p["modifier"])
        if unit_type is None:
            continue

        amount = float(p["amount"])
        gram_weight = float(p["gram_weight"])
        if not all(math.isfinite(v) and v > 0 for v in (amount, gram_weight)):
            raise ValueError(f"Invalid volume portion: {p}")

        # Calculate grams per single unit
        grams_per_unit = gram_weight / amount
        if not math.isfinite(grams_per_unit * TSP_PER_CUP):
            raise ValueError(f"Overflow in volume portion: {p}")

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


def build_data(foods: list[dict], portions: list[dict]) -> dict:
    """Use the pinned archive's row order to resolve normalized-name collisions."""
    if not foods or not portions:
        raise ValueError("Expected nonempty USDA food and portion tables")
    ingredients = get_curated_ingredients()
    food_map = {}
    for food in foods:
        food_id = food["fdc_id"]
        if int(food_id) <= 0 or food_id in food_map or not food["description"].strip():
            raise ValueError(f"Invalid/duplicate food: {food}")
        food_map[food_id] = food["description"]

    # Group portions by food
    portions_by_food = defaultdict(list)
    portion_ids = set()
    for p in portions:
        if int(p["id"]) <= 0 or p["id"] in portion_ids:
            raise ValueError(f"Invalid/duplicate portion: {p}")
        portion_ids.add(p["id"])
        if p["fdc_id"] not in food_map:
            raise ValueError(f"Unknown food in portion: {p}")
        portions_by_food[p["fdc_id"]].append(p)

    # Calculate grams per cup for each food from USDA
    usda_count = 0

    aliases = get_curated_aliases()
    generated_aliases = {}

    for fdc_id, food_portions in portions_by_food.items():
        description = food_map[fdc_id]

        # Calculate grams per cup
        grams_per_cup = calculate_grams_per_cup(food_portions)
        if grams_per_cup is None:
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

    if usda_count == 0:
        raise ValueError("No USDA volume densities found")

    # Merge generated aliases into curated aliases
    all_aliases = {**aliases, **generated_aliases}

    # Build JSON structure
    if any(not math.isfinite(v) or v <= 0 for v in ingredients.values()):
        raise ValueError("Invalid generated density")
    if any(target not in ingredients for target in all_aliases.values()):
        raise ValueError("Alias target missing from ingredients")
    return {
        "source": SOURCE,
        "archive_sha256": ARCHIVE_SHA256,
        "release": "USDA SR Legacy April 2018",
        "excluded_zero_amount_portion_ids": sorted(EXCLUDED_PORTIONS, key=int),
        "generation_notes": (
            "See ingredient-density/README.md for selection rules "
            "and embedded manual values."
        ),
        "ingredients": dict(sorted(ingredients.items())),
        "aliases": dict(sorted(all_aliases.items())),
    }


def main() -> None:
    contents = load_archive(CACHE)
    with zipfile.ZipFile(io.BytesIO(contents)) as archive:
        data = build_data(
            read_rows(archive, "food.csv"), read_rows(archive, "food_portion.csv")
        )
    serialized = json.dumps(data, indent=2, allow_nan=False) + "\n"
    OUTPUT.write_text(serialized, encoding="utf-8")
    print(
        f"Imported {len(data['ingredients'])} densities into {OUTPUT.relative_to(ROOT)}"
    )


if __name__ == "__main__":
    main()
