#!/usr/bin/env python3
"""
Import the pinned USDA SR Legacy release into the ingredient catalog.

Writes ramekin-core/src/catalog/data/usda.json: one record per food with its
energy and volume density, plus the stripped-name index the resolver uses.
Hand-maintained entries, aliases, and citations live in curated.json; this
script is a pure projection of the USDA archive.

Usage:
    make catalog-import
"""

import csv
import hashlib
import io
import json
import math
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = "https://fdc.nal.usda.gov/fdc-datasets/FoodData_Central_sr_legacy_food_csv_2018-04.zip"
ARCHIVE_SHA256 = "b80817294b8850530aaedf2e515c02593b1824f763a0ff356e5c2081643e6fd0"
CACHE = ROOT / ".cache/nutrition-sr-legacy-2018-04.zip"
OUTPUT = ROOT / "ramekin-core/src/catalog/data/usda.json"
ENERGY_KCAL_NUTRIENT_ID = "1008"
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
TBSP_PER_CUP = 16.0
TSP_PER_CUP = 48.0
# Suffixes that carry no meaning in recipe ingredient names.
NAME_SUFFIXES = [
    ", enriched, bleached",
    ", enriched, unbleached",
    ", enriched",
    ", unenriched",
    ", raw",
    ", dry",
    " (includes foods for usda's food distribution program)",
]


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
    """Return 'cup', 'tbsp', 'tsp', or None for an SR Legacy portion modifier."""
    mod = modifier.lower().strip()
    # "cup chips" and similar are not a typical measurement.
    if "chip" in mod:
        return None
    if mod in ("cup", "cups") or mod.startswith("cup,") or mod.startswith("cup ("):
        return "cup"
    if mod in ("tbsp", "tablespoon"):
        return "tbsp"
    if mod in ("tsp", "teaspoon"):
        return "tsp"
    return None


def calculate_grams_per_cup(portions: list[dict]) -> float | None:
    """Average cup portions; fall back to tablespoons, then teaspoons."""
    by_unit: dict[str, list[float]] = {"cup": [], "tbsp": [], "tsp": []}
    per_cup = {"cup": 1.0, "tbsp": TBSP_PER_CUP, "tsp": TSP_PER_CUP}
    for p in portions:
        if p["id"] in EXCLUDED_PORTIONS:
            values = tuple(
                p[key] for key in ("fdc_id", "modifier", "amount", "gram_weight")
            )
            if values != EXCLUDED_PORTIONS[p["id"]]:
                raise ValueError(f"Excluded USDA portion changed: {p}")
            continue
        unit = parse_volume_unit(p["modifier"])
        if unit is None:
            continue
        amount = float(p["amount"])
        gram_weight = float(p["gram_weight"])
        if not all(math.isfinite(v) and v > 0 for v in (amount, gram_weight)):
            raise ValueError(f"Invalid volume portion: {p}")
        grams_per_unit = gram_weight / amount
        if not math.isfinite(grams_per_unit * TSP_PER_CUP):
            raise ValueError(f"Overflow in volume portion: {p}")
        by_unit[unit].append(grams_per_unit * per_cup[unit])
    for unit in ("cup", "tbsp", "tsp"):
        if by_unit[unit]:
            return sum(by_unit[unit]) / len(by_unit[unit])
    return None


def normalize(text: str) -> str:
    """Lowercase and collapse whitespace, matching the resolver's normalization."""
    return " ".join(text.lower().split())


def strip_name(description: str) -> str:
    """Lowercase a USDA description and drop meaningless trailing suffixes.

    Suffixes are checked in list order, so "x, dry, raw" loses both.
    """
    name = normalize(description)
    for suffix in NAME_SUFFIXES:
        if name.endswith(suffix):
            name = name[: -len(suffix)]
    return name


def build_data(foods: list[dict], nutrients: list[dict], portions: list[dict]) -> dict:
    if not foods or not portions:
        raise ValueError("Expected nonempty USDA food and portion tables")
    descriptions = {}
    for food in foods:
        food_id = food["fdc_id"]
        if (
            int(food_id) <= 0
            or food_id in descriptions
            or not food["description"].strip()
        ):
            raise ValueError(f"Invalid/duplicate food: {food}")
        descriptions[food_id] = food["description"]

    energy = {}
    for row in nutrients:
        if row["nutrient_id"] != ENERGY_KCAL_NUTRIENT_ID:
            continue
        food_id = row["fdc_id"]
        amount = float(row["amount"])
        if food_id in energy or not math.isfinite(amount) or amount < 0:
            raise ValueError(f"Invalid/duplicate energy for {food_id}")
        if food_id not in descriptions:
            raise ValueError(f"Unknown food in nutrient: {row}")
        energy[food_id] = amount

    # Group portions by food in portion-table order; that order also decides
    # which food owns a stripped name when several share one.
    portions_by_food: dict[str, list[dict]] = {}
    portion_ids = set()
    for p in portions:
        if int(p["id"]) <= 0 or p["id"] in portion_ids:
            raise ValueError(f"Invalid/duplicate portion: {p}")
        portion_ids.add(p["id"])
        if p["fdc_id"] not in descriptions:
            raise ValueError(f"Unknown food in portion: {p}")
        portions_by_food.setdefault(p["fdc_id"], []).append(p)

    densities = {}
    for food_id, food_portions in portions_by_food.items():
        grams_per_cup = calculate_grams_per_cup(food_portions)
        if grams_per_cup is not None:
            densities[food_id] = grams_per_cup
    if not densities:
        raise ValueError("No USDA volume densities found")

    # A stripped name belongs to the first food with a density (portion-table
    # order), else to the first food in food-table order.
    names: dict[str, int] = {}
    for food_id in densities:
        names.setdefault(strip_name(descriptions[food_id]), int(food_id))
    for food in foods:
        names.setdefault(strip_name(food["description"]), int(food["fdc_id"]))

    records = [
        {
            "fdc_id": int(food_id),
            "description": normalize(description),
            "kcal_per_100g": energy.get(food_id),
            "grams_per_cup": densities.get(food_id),
        }
        for food_id, description in descriptions.items()
    ]
    return {
        "source": SOURCE,
        "archive_sha256": ARCHIVE_SHA256,
        "release": "USDA SR Legacy April 2018",
        "excluded_zero_amount_portion_ids": sorted(EXCLUDED_PORTIONS, key=int),
        "generation_notes": (
            "Generated by scripts/import-catalog.py (make catalog-import). "
            "See ramekin-core/src/catalog/README.md for selection rules."
        ),
        "foods": sorted(records, key=lambda record: record["fdc_id"]),
        "names": dict(sorted(names.items())),
    }


def main() -> None:
    contents = load_archive(CACHE)
    with zipfile.ZipFile(io.BytesIO(contents)) as archive:
        data = build_data(
            read_rows(archive, "food.csv"),
            read_rows(archive, "food_nutrient.csv"),
            read_rows(archive, "food_portion.csv"),
        )
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(
        json.dumps(data, indent=2, ensure_ascii=False, allow_nan=False) + "\n",
        encoding="utf-8",
    )
    print(f"Imported {len(data['foods'])} foods into {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
