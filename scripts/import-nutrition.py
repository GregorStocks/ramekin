"""Import the fixed SR Legacy release, keeping food IDs and energy provenance."""

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
OUTPUT = ROOT / "ramekin-core/src/nutrition/data.json"


def read_rows(archive: zipfile.ZipFile, filename: str) -> list[dict[str, str]]:
    matches = [name for name in archive.namelist() if name.endswith("/" + filename)]
    if len(matches) != 1:
        raise ValueError(f"Expected exactly one {filename}: {matches}")
    with archive.open(matches[0]) as stream:
        return list(csv.DictReader(io.TextIOWrapper(stream, encoding="utf-8-sig")))


def main() -> None:
    CACHE.parent.mkdir(parents=True, exist_ok=True)
    if not CACHE.exists():
        with urllib.request.urlopen(SOURCE, timeout=120) as response:
            contents = response.read()
        with zipfile.ZipFile(io.BytesIO(contents)) as archive:
            if archive.testzip() is not None:
                raise ValueError("Invalid USDA archive")
        CACHE.write_bytes(contents)
    archive_bytes = CACHE.read_bytes()
    if hashlib.sha256(archive_bytes).hexdigest() != ARCHIVE_SHA256:
        raise ValueError(
            "USDA archive checksum mismatch; review source before updating"
        )
    with zipfile.ZipFile(io.BytesIO(archive_bytes)) as archive:
        foods = read_rows(archive, "food.csv")
        nutrients = read_rows(archive, "food_nutrient.csv")
        portions = read_rows(archive, "food_portion.csv")
    cup_weights: dict[str, set[float]] = {}
    for row in portions:
        if row["modifier"] == "cup" and float(row["amount"]) > 0:
            grams = float(row["gram_weight"]) / float(row["amount"])
            if not math.isfinite(grams) or grams <= 0:
                raise ValueError(f"Invalid cup weight: {row}")
            cup_weights.setdefault(row["fdc_id"], set()).add(grams)
    energy = {}
    for row in nutrients:
        if row["nutrient_id"] != "1008":
            continue
        food_id = row["fdc_id"]
        amount = float(row["amount"])
        if food_id in energy or not math.isfinite(amount) or amount < 0:
            raise ValueError(f"Invalid/duplicate energy for {food_id}")
        energy[food_id] = amount
    records = [
        {
            "fdc_id": int(food["fdc_id"]),
            "name": food["description"].lower(),
            "kcal_per_100g": energy[food["fdc_id"]],
            "grams_per_cup": (
                next(iter(cup_weights[food["fdc_id"]]))
                if len(cup_weights.get(food["fdc_id"], set())) == 1
                else None
            ),
        }
        for food in foods
        if food["fdc_id"] in energy
    ]
    result = {
        "source": SOURCE,
        "archive_sha256": hashlib.sha256(archive_bytes).hexdigest(),
        "release": "USDA SR Legacy April 2018",
        "foods": sorted(records, key=lambda food: food["fdc_id"]),
    }
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")
    print(f"Imported {len(records)} foods into {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
