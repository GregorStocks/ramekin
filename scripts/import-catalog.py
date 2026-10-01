#!/usr/bin/env python3
"""
Import the pinned USDA releases into the ingredient catalog.

Writes ramekin-core/src/catalog/data/usda.json from SR Legacy: one record per
food with its energy, volume density, and per-piece weights, plus the
stripped-name index the resolver uses.

Writes ramekin-core/src/catalog/data/fndds.json from FNDDS (survey foods), a
secondary source for foods SR Legacy lacks: energy and volume density only,
and no name index, so its foods are reachable only through curated entries.

Hand-maintained entries, aliases, and citations live in curated.json; this
script is a pure projection of the USDA archives.

Usage:
    make catalog-import
"""

import csv
import hashlib
import io
import json
import math
import re
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = "https://fdc.nal.usda.gov/fdc-datasets/FoodData_Central_sr_legacy_food_csv_2018-04.zip"
ARCHIVE_SHA256 = "b80817294b8850530aaedf2e515c02593b1824f763a0ff356e5c2081643e6fd0"
CACHE = ROOT / ".cache/nutrition-sr-legacy-2018-04.zip"
OUTPUT = ROOT / "ramekin-core/src/catalog/data/usda.json"
ENERGY_KCAL_NUTRIENT_ID = "1008"
FNDDS_SOURCE = "https://fdc.nal.usda.gov/fdc-datasets/FoodData_Central_survey_food_csv_2024-10-31.zip"
FNDDS_ARCHIVE_SHA256 = (
    "5ccc25ec2777a8982fbb61378a42f415316173eb11e48c9a8ba4cb19f5a4f29c"
)
FNDDS_CACHE = ROOT / ".cache/nutrition-fndds-2024-10-31.zip"
FNDDS_OUTPUT = ROOT / "ramekin-core/src/catalog/data/fndds.json"
# FNDDS food_nutrient rows use the nutrient number, not the nutrient id.
FNDDS_ENERGY_KCAL_NUTRIENT_NUMBER = "208"
# An FNDDS portion description that measures volume: "1 cup", "1/2 cup, diced",
# "1 1/2 tablespoons", "1 fl oz (no ice)".
FNDDS_VOLUME = re.compile(
    r"^(\d+(?:\.\d+)?|\d+/\d+|\d+ \d+/\d+) "
    r"(cups?|tablespoons?|teaspoons?|fl oz)(?:$|[,( ])"
)
FNDDS_UNITS = {
    "cup": "cup",
    "cups": "cup",
    "tablespoon": "tbsp",
    "tablespoons": "tbsp",
    "teaspoon": "tsp",
    "teaspoons": "tsp",
    "fl oz": "fl oz",
}
FL_OZ_PER_CUP = 8.0
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
# Portion modifiers that measure volume, mass, or a package rather than a piece.
NON_PIECE_MODIFIERS = {
    "cup",
    "cups",
    "tbsp",
    "tablespoon",
    "tablespoons",
    "tsp",
    "teaspoon",
    "teaspoons",
    "fl oz",
    "oz",
    "lb",
    "pint",
    "quart",
    "gallon",
    "g",
    "ml",
    "liter",
    "cubic inch",
    "serving",
    "nlea serving",
    "package",
    "can",
    "can or bottle",
    "container",
    "jar",
    "bottle",
    "bag",
    "packet",
    "block",
    "box",
    "carton",
    "tin",
    "tub",
    "scoop",
    "portion",
    "unit",
    "item",
}
SIZE_WORDS = {"small", "medium", "large", "extra large", "jumbo"}
# Preferred piece for a bare count, before falling back to "<piece> medium" or
# the only piece.
# Parts of a piece, which a bare count never means.
PARTIAL_PIECES = {"slice", "strip", "wedge", "ring", "cube", "chip", "pat", "spear"}
DEFAULT_PORTION_ORDER = ["medium", "fruit", "whole", "large", "small"]
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


def verify_archive(contents: bytes, sha256: str) -> None:
    if hashlib.sha256(contents).hexdigest() != sha256:
        raise ValueError(
            "USDA archive checksum mismatch; review source before updating"
        )


def load_archive(cache: Path, source: str, sha256: str) -> bytes:
    if cache.exists():
        contents = cache.read_bytes()
        verify_archive(contents, sha256)
        return contents
    with urllib.request.urlopen(source, timeout=120) as response:
        contents = response.read()
    verify_archive(contents, sha256)
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


def singular(word: str) -> str:
    """Singularize a piece name the way catalog::piece_unit does."""
    if word.endswith("leaves"):
        return word[: -len("leaves")] + "leaf"
    if word.endswith("s") and not word.endswith("ss") and len(word) > 3:
        return word[:-1]
    return word


def portion_key(modifier: str) -> str | None:
    """Normalize a piece portion's modifier ("medium (2-1/2" dia)" -> "medium",
    "slice, large (1/4" thick)" -> "slice large", "cloves" -> "clove").

    Volume, mass, and package measures are not pieces, and neither are the
    cooked yield of a pound of meat or a whole recipe; all return None.
    """
    if re.search(r"yield from 1 lb|recipe yield", modifier.lower()):
        return None
    text = normalize(re.sub(r"\([^)]*\)?", " ", modifier))
    parts = [part.strip() for part in text.split(",") if part.strip()]
    if not parts:
        return None
    head = parts[0]
    if (
        head in NON_PIECE_MODIFIERS
        or singular(head.split(" ")[0]) in NON_PIECE_MODIFIERS
        or any(char.isdigit() for char in head)
    ):
        return None
    words = head.split(" ")
    # "large whole" and "slice raw" are the plain "large" and "slice".
    if len(words) > 1 and words[-1] in ("whole", "raw"):
        words.pop()
    head = " ".join([singular(words[0]), *words[1:]])
    if len(parts) > 1 and parts[1] in SIZE_WORDS:
        head = f"{head} {parts[1]}"
    return head


def piece_portions(portions: list[dict]) -> dict[str, float]:
    """Grams per single piece, keyed by portion_key. The first portion in
    portion-table order wins when two share a key."""
    pieces: dict[str, float] = {}
    for p in portions:
        key = portion_key(p["modifier"])
        amount = float(p["amount"])
        # Zero-amount rows are yield notes ("chicken, skin only"), not pieces.
        if key is None or key in pieces or amount == 0:
            continue
        gram_weight = float(p["gram_weight"])
        if not all(math.isfinite(v) and v > 0 for v in (amount, gram_weight)):
            raise ValueError(f"Invalid piece portion: {p}")
        pieces[key] = gram_weight / amount
    return pieces


def default_portion(pieces: dict[str, float]) -> str | None:
    """The piece a bare count ("3 carrots") most likely means. Parts of a
    piece (a slice, a wedge) never are."""
    whole = [key for key in pieces if key.split(" ")[0] not in PARTIAL_PIECES]
    for key in DEFAULT_PORTION_ORDER:
        if key in whole:
            return key
    sized = sorted(key for key in whole if key.endswith(" medium"))
    if sized:
        return sized[0]
    if len(whole) == 1:
        return whole[0]
    return None


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

    pieces = {
        food_id: piece_portions(portions_by_food.get(food_id, []))
        for food_id in descriptions
    }

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
            "portions": dict(sorted(pieces[food_id].items())),
            "default_portion": default_portion(pieces[food_id]),
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


def parse_amount(text: str) -> float:
    """ "1", "0.5", "1/2", or "1 1/2" as a number."""
    total = 0.0
    for part in text.split(" "):
        if "/" in part:
            numerator, denominator = part.split("/")
            total += float(numerator) / float(denominator)
        else:
            total += float(part)
    return total


def fndds_grams_per_cup(portions: list[dict]) -> float | None:
    """Average an FNDDS food's cup portions; fall back to tablespoons, then
    teaspoons, then fluid ounces."""
    by_unit: dict[str, list[float]] = {"cup": [], "tbsp": [], "tsp": [], "fl oz": []}
    per_cup = {
        "cup": 1.0,
        "tbsp": TBSP_PER_CUP,
        "tsp": TSP_PER_CUP,
        "fl oz": FL_OZ_PER_CUP,
    }
    for p in portions:
        match = FNDDS_VOLUME.match(normalize(p["portion_description"]))
        if match is None:
            continue
        amount = parse_amount(match.group(1))
        gram_weight = float(p["gram_weight"])
        if not all(math.isfinite(v) and v > 0 for v in (amount, gram_weight)):
            raise ValueError(f"Invalid FNDDS volume portion: {p}")
        unit = FNDDS_UNITS[match.group(2)]
        by_unit[unit].append(gram_weight / amount * per_cup[unit])
    for unit in ("cup", "tbsp", "tsp", "fl oz"):
        if by_unit[unit]:
            return sum(by_unit[unit]) / len(by_unit[unit])
    return None


def build_fndds_data(
    foods: list[dict], nutrients: list[dict], portions: list[dict]
) -> dict:
    """Project FNDDS into the same food records as SR Legacy, without piece
    weights or a name index."""
    if not foods or not portions:
        raise ValueError("Expected nonempty FNDDS food and portion tables")
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
        if row["nutrient_id"] != FNDDS_ENERGY_KCAL_NUTRIENT_NUMBER:
            continue
        food_id = row["fdc_id"]
        amount = float(row["amount"])
        if food_id in energy or not math.isfinite(amount) or amount < 0:
            raise ValueError(f"Invalid/duplicate energy for {food_id}")
        if food_id not in descriptions:
            raise ValueError(f"Unknown food in nutrient: {row}")
        energy[food_id] = amount

    portions_by_food: dict[str, list[dict]] = {}
    portion_ids = set()
    for p in portions:
        if int(p["id"]) <= 0 or p["id"] in portion_ids:
            raise ValueError(f"Invalid/duplicate portion: {p}")
        portion_ids.add(p["id"])
        if p["fdc_id"] not in descriptions:
            raise ValueError(f"Unknown food in portion: {p}")
        portions_by_food.setdefault(p["fdc_id"], []).append(p)

    records = [
        {
            "fdc_id": int(food_id),
            "description": normalize(description),
            "kcal_per_100g": energy.get(food_id),
            "grams_per_cup": fndds_grams_per_cup(portions_by_food.get(food_id, [])),
            "portions": {},
            "default_portion": None,
        }
        for food_id, description in descriptions.items()
    ]
    return {
        "source": FNDDS_SOURCE,
        "archive_sha256": FNDDS_ARCHIVE_SHA256,
        "release": "USDA FNDDS (survey foods) October 2024",
        "generation_notes": (
            "Generated by scripts/import-catalog.py (make catalog-import). "
            "See ramekin-core/src/catalog/README.md for selection rules."
        ),
        "foods": sorted(records, key=lambda record: record["fdc_id"]),
    }


def write_json(path: Path, data: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(data, indent=2, ensure_ascii=False, allow_nan=False) + "\n",
        encoding="utf-8",
    )


def read_tables(contents: bytes) -> tuple[list[dict], list[dict], list[dict]]:
    with zipfile.ZipFile(io.BytesIO(contents)) as archive:
        return (
            read_rows(archive, "food.csv"),
            read_rows(archive, "food_nutrient.csv"),
            read_rows(archive, "food_portion.csv"),
        )


def main() -> None:
    sr = build_data(*read_tables(load_archive(CACHE, SOURCE, ARCHIVE_SHA256)))
    fndds = build_fndds_data(
        *read_tables(load_archive(FNDDS_CACHE, FNDDS_SOURCE, FNDDS_ARCHIVE_SHA256))
    )
    overlap = {food["fdc_id"] for food in sr["foods"]} & {
        food["fdc_id"] for food in fndds["foods"]
    }
    if overlap:
        raise ValueError(f"SR Legacy and FNDDS share FDC ids: {sorted(overlap)[:10]}")
    write_json(OUTPUT, sr)
    write_json(FNDDS_OUTPUT, fndds)
    print(f"Imported {len(sr['foods'])} foods into {OUTPUT.relative_to(ROOT)}")
    print(f"Imported {len(fndds['foods'])} foods into {FNDDS_OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
