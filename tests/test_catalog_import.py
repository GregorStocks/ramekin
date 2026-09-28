"""Offline regression tests for the USDA ingredient catalog import command."""

import hashlib
import importlib.util
import io
import json
import zipfile
from pathlib import Path

import pytest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts/import-catalog.py"
SPEC = importlib.util.spec_from_file_location("catalog_import", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
IMPORTER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(IMPORTER)


def portion(modifier="cup", amount="1", weight="120", food_id="1", row_id="1"):
    return {
        "id": row_id,
        "fdc_id": food_id,
        "modifier": modifier,
        "amount": amount,
        "gram_weight": weight,
    }


def energy(food_id="1", kcal="100"):
    return {"fdc_id": food_id, "nutrient_id": "1008", "amount": kcal}


@pytest.mark.parametrize(
    ("rows", "expected"),
    [
        ([portion(amount="0.5", weight="60")], 120),
        ([portion("tbsp", weight="10")], 160),
        ([portion("teaspoon", weight="2")], 96),
        ([portion("cup, diced", weight="100"), portion(weight="140")], 120),
        ([portion(), portion("tbsp", weight="10")], 120),
        ([portion("tbsp", weight="10"), portion("tsp", weight="2")], 160),
        ([portion("piece"), portion("cup chips")], None),
    ],
)
def test_portion_conversion(rows, expected):
    assert IMPORTER.calculate_grams_per_cup(rows) == expected


@pytest.mark.parametrize("field", ["amount", "gram_weight"])
@pytest.mark.parametrize("value", ["0", "-1", "nan", "inf", "", "bad"])
def test_invalid_volume_fails(field, value):
    row = portion()
    row[field] = value
    with pytest.raises(ValueError):
        IMPORTER.calculate_grams_per_cup([row])


def test_known_zero_amount_exclusions_are_exact():
    for row_id, (
        food_id,
        modifier,
        amount,
        weight,
    ) in IMPORTER.EXCLUDED_PORTIONS.items():
        row = portion(modifier, amount, weight, food_id, row_id)
        assert IMPORTER.calculate_grams_per_cup([row, portion()]) == 120
        row["amount"] = "1"
        with pytest.raises(ValueError, match="Excluded USDA portion changed"):
            IMPORTER.calculate_grams_per_cup([row])


def test_overflow_fails():
    with pytest.raises(ValueError, match="Overflow"):
        IMPORTER.calculate_grams_per_cup([portion(amount="1e-300", weight="1e300")])


@pytest.mark.parametrize(
    ("description", "expected"),
    [
        ("Example, raw", "example"),
        (
            "Wheat flour, white, all-purpose, enriched, bleached",
            "wheat flour, white, all-purpose",
        ),
        ("Example, dry, raw", "example"),
        ("Fruit  strips,  RAW", "fruit strips"),
    ],
)
def test_strip_name(description, expected):
    assert IMPORTER.strip_name(description) == expected


def test_generation_records_and_names():
    foods = [
        {"fdc_id": "1", "description": "Example, raw"},
        {"fdc_id": "2", "description": "Example"},
        {"fdc_id": "3", "description": "Butter,  salted"},
        {"fdc_id": "4", "description": "No density, raw"},
    ]
    # Portion-table order decides the stripped-name owner among foods with a
    # density: food 2 appears first, so it owns "example".
    rows = [
        portion(food_id="2", row_id="1", weight="200"),
        portion(food_id="1", row_id="2", weight="100"),
        portion(food_id="3", row_id="3", weight="300"),
        portion("piece", food_id="4", row_id="4"),
    ]
    nutrients = [energy("1", "10"), energy("2", "20"), energy("3", "717")]
    data = IMPORTER.build_data(foods, nutrients, rows)
    records = {record["fdc_id"]: record for record in data["foods"]}
    assert records[3] == {
        "fdc_id": 3,
        "description": "butter, salted",
        "kcal_per_100g": 717.0,
        "grams_per_cup": 300.0,
    }
    assert records[4]["kcal_per_100g"] is None
    assert records[4]["grams_per_cup"] is None
    assert data["names"] == {
        "butter, salted": 3,
        "example": 2,
        "no density": 4,
    }
    assert [record["fdc_id"] for record in data["foods"]] == [1, 2, 3, 4]
    assert list(data["names"]) == sorted(data["names"])
    assert data["archive_sha256"] == IMPORTER.ARCHIVE_SHA256


@pytest.mark.parametrize(
    ("foods", "nutrients", "rows"),
    [
        ([], [], [portion()]),
        ([{"fdc_id": "1", "description": "Example"}], [], []),
        ([{"fdc_id": "1", "description": "Example"}] * 2, [], [portion()]),
        ([{"fdc_id": "1", "description": " "}], [], [portion()]),
        ([{"fdc_id": "2", "description": "Example"}], [], [portion()]),
        ([{"fdc_id": "1", "description": "Example"}], [], [portion()] * 2),
        ([{"fdc_id": "1", "description": "Example"}], [], [portion("piece")]),
        ([{"fdc_id": "1", "description": "Example"}], [energy("2")], [portion()]),
        ([{"fdc_id": "1", "description": "Example"}], [energy()] * 2, [portion()]),
        ([{"fdc_id": "1", "description": "Example"}], [energy(kcal="-1")], [portion()]),
    ],
)
def test_invalid_tables_fail(foods, nutrients, rows):
    with pytest.raises(ValueError):
        IMPORTER.build_data(foods, nutrients, rows)


def archive_bytes(members):
    stream = io.BytesIO()
    with zipfile.ZipFile(stream, "w") as archive:
        for name, contents in members.items():
            archive.writestr(name, contents)
    return stream.getvalue()


def test_cached_regeneration_is_reproducible(tmp_path, monkeypatch):
    contents = archive_bytes(
        {
            "release/food.csv": '﻿fdc_id,description\n1,"Example, raw"\n',
            "release/food_nutrient.csv": "fdc_id,nutrient_id,amount\n1,1008,52\n",
            "release/food_portion.csv": (
                "id,fdc_id,modifier,amount,gram_weight\n1,1,cup,0.5,60\n"
            ),
        }
    )
    cache = tmp_path / "source.zip"
    cache.write_bytes(contents)
    output = tmp_path / "usda.json"
    monkeypatch.setattr(
        IMPORTER, "ARCHIVE_SHA256", hashlib.sha256(contents).hexdigest()
    )
    monkeypatch.setattr(IMPORTER, "CACHE", cache)
    monkeypatch.setattr(IMPORTER, "OUTPUT", output)
    monkeypatch.setattr(IMPORTER, "ROOT", tmp_path)
    IMPORTER.main()
    first = output.read_bytes()
    IMPORTER.main()
    assert output.read_bytes() == first
    data = json.loads(first)
    assert data["foods"] == [
        {
            "fdc_id": 1,
            "description": "example, raw",
            "kcal_per_100g": 52.0,
            "grams_per_cup": 120.0,
        }
    ]
    assert data["names"] == {"example": 1}
    cache.write_bytes(b"corrupt archive")
    with pytest.raises(ValueError, match="checksum mismatch"):
        IMPORTER.main()
    assert output.read_bytes() == first


def test_download_validated_before_caching(tmp_path, monkeypatch):
    cache = tmp_path / "source.zip"
    contents = b"downloaded archive"
    monkeypatch.setattr(
        IMPORTER.urllib.request, "urlopen", lambda *a, **kw: io.BytesIO(contents)
    )
    with pytest.raises(ValueError, match="checksum mismatch"):
        IMPORTER.load_archive(cache)
    assert not cache.exists()
    monkeypatch.setattr(
        IMPORTER, "ARCHIVE_SHA256", hashlib.sha256(contents).hexdigest()
    )
    assert IMPORTER.load_archive(cache) == contents
    assert cache.read_bytes() == contents


@pytest.mark.parametrize("members", [{}, {"a/food.csv": "", "b/food.csv": ""}])
def test_missing_or_ambiguous_csv_fails(members):
    with zipfile.ZipFile(io.BytesIO(archive_bytes(members))) as archive:
        with pytest.raises(ValueError, match="exactly one food.csv"):
            IMPORTER.read_rows(archive, "food.csv")
