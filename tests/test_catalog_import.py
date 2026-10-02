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
        # Liquids weighed only per fluid ounce; cups and spoons win when present.
        ([portion("fl oz", weight="29.5")], 236),
        ([portion("fl oz", amount="1.5", weight="42")], 224),
        ([portion("fl oz", weight="30"), portion("tbsp", weight="15")], 240),
        ([portion("serving (5 fl oz)", weight="147")], None),
        ([portion("jigger (1.5 fl oz)", weight="42")], None),
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
        "portions": {},
        "default_portion": None,
    }
    assert records[4]["kcal_per_100g"] is None
    assert records[4]["grams_per_cup"] is None
    assert records[4]["portions"] == {"piece": 120.0}
    assert records[4]["default_portion"] == "piece"
    assert data["names"] == {
        "butter, salted": 3,
        "example": 2,
        "no density": 4,
    }
    assert [record["fdc_id"] for record in data["foods"]] == [1, 2, 3, 4]
    assert list(data["names"]) == sorted(data["names"])
    assert data["archive_sha256"] == IMPORTER.ARCHIVE_SHA256


@pytest.mark.parametrize(
    ("modifier", "expected"),
    [
        ('medium (2-1/2" dia)', "medium"),
        ('slice, large (1/4" thick)', "slice large"),
        ('stalk, medium (7-1/2" - 8" long)', "stalk medium"),
        ("cloves", "clove"),
        ("leaves", "leaf"),
        ('large whole (3" dia)', "large"),
        ("slice raw", "slice"),
        ("chop, excluding refuse (yield from 1 raw chop)", "chop"),
        ("extra large", "extra large"),
        ("glass", "glass"),
        ("cup, chopped", None),
        ("cup slices", None),
        ("oz, boneless", None),
        ("package (10 oz)", None),
        ("carton", None),
        ("block", None),
        ("blocks", None),
        ("NLEA serving", None),
        ("jar Gerber Second Food (4 oz)", None),
        ("10 pieces", None),
        ("piece, cooked (yield from 1 lb raw meat)", None),
        ("recipe yield", None),
    ],
)
def test_portion_key(modifier, expected):
    assert IMPORTER.portion_key(modifier) == expected


def test_piece_portions_are_per_one_and_first_wins():
    rows = [
        portion("cloves", amount="3", weight="9"),
        portion("fruit", weight="58", row_id="2"),
        portion("fruit", weight="84", row_id="3"),
        portion("chicken, skin only", amount="0", weight="72", row_id="4"),
        portion("cup", weight="136", row_id="5"),
    ]
    assert IMPORTER.piece_portions(rows) == {"clove": 3.0, "fruit": 58.0}
    with pytest.raises(ValueError):
        IMPORTER.piece_portions([portion("piece", weight="0")])


@pytest.mark.parametrize(
    ("pieces", "expected"),
    [
        ({"large": 1, "medium": 1, "small": 1}, "medium"),
        ({"fruit": 1, "large": 1}, "fruit"),
        ({"large": 1, "small": 1}, "large"),
        ({"potato large": 1, "potato medium": 1}, "potato medium"),
        ({"clove": 1}, "clove"),
        ({"slice": 1, "slice medium": 1, "medium": 1}, "medium"),
        ({"slice": 1}, None),
        ({"stick": 1, "pat": 1}, "stick"),
        ({"cookie": 1, "bar": 1}, None),
        ({}, None),
    ],
)
def test_default_portion(pieces, expected):
    assert IMPORTER.default_portion(pieces) == expected


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
    fndds_contents = archive_bytes(
        {
            "release/food.csv": 'fdc_id,description\n2,"Simple syrup"\n',
            "release/food_nutrient.csv": "fdc_id,nutrient_id,amount\n2,208,184\n",
            "release/food_portion.csv": (
                "id,fdc_id,portion_description,gram_weight\n1,2,1 tablespoon,20\n"
            ),
        }
    )
    fndds_cache = tmp_path / "fndds.zip"
    fndds_cache.write_bytes(fndds_contents)
    fndds_output = tmp_path / "fndds.json"
    monkeypatch.setattr(
        IMPORTER, "ARCHIVE_SHA256", hashlib.sha256(contents).hexdigest()
    )
    monkeypatch.setattr(
        IMPORTER, "FNDDS_ARCHIVE_SHA256", hashlib.sha256(fndds_contents).hexdigest()
    )
    monkeypatch.setattr(IMPORTER, "CACHE", cache)
    monkeypatch.setattr(IMPORTER, "OUTPUT", output)
    monkeypatch.setattr(IMPORTER, "FNDDS_CACHE", fndds_cache)
    monkeypatch.setattr(IMPORTER, "FNDDS_OUTPUT", fndds_output)
    monkeypatch.setattr(IMPORTER, "ROOT", tmp_path)
    IMPORTER.main()
    first = output.read_bytes()
    first_fndds = fndds_output.read_bytes()
    IMPORTER.main()
    assert output.read_bytes() == first
    assert fndds_output.read_bytes() == first_fndds
    assert json.loads(first_fndds)["foods"] == [
        {
            "fdc_id": 2,
            "description": "simple syrup",
            "kcal_per_100g": 184.0,
            "grams_per_cup": 320.0,
            "portions": {},
            "default_portion": None,
        }
    ]
    data = json.loads(first)
    assert data["foods"] == [
        {
            "fdc_id": 1,
            "description": "example, raw",
            "kcal_per_100g": 52.0,
            "grams_per_cup": 120.0,
            "portions": {},
            "default_portion": None,
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
        IMPORTER.load_archive(cache, "https://example.test/a.zip", "0" * 64)
    assert not cache.exists()
    sha256 = hashlib.sha256(contents).hexdigest()
    assert (
        IMPORTER.load_archive(cache, "https://example.test/a.zip", sha256) == contents
    )
    assert cache.read_bytes() == contents


@pytest.mark.parametrize("members", [{}, {"a/food.csv": "", "b/food.csv": ""}])
def test_missing_or_ambiguous_csv_fails(members):
    with zipfile.ZipFile(io.BytesIO(archive_bytes(members))) as archive:
        with pytest.raises(ValueError, match="exactly one food.csv"):
            IMPORTER.read_rows(archive, "food.csv")


def fndds_portion(description, weight="100", food_id="1", row_id="1"):
    return {
        "id": row_id,
        "fdc_id": food_id,
        "portion_description": description,
        "gram_weight": weight,
    }


@pytest.mark.parametrize(
    ("rows", "expected"),
    [
        ([fndds_portion("1 cup", "240")], 240),
        ([fndds_portion("1/2 cup, diced", "60")], 120),
        ([fndds_portion("1 1/2 cups", "300")], 200),
        ([fndds_portion("1 tablespoon", "15")], 240),
        ([fndds_portion("2 teaspoons", "10")], 240),
        ([fndds_portion("1 fl oz (no ice)", "30")], 240),
        # Cups win over spoons; other measures are not volumes.
        ([fndds_portion("1 tablespoon", "10"), fndds_portion("1 cup", "200")], 200),
        ([fndds_portion("Quantity not specified", "0")], None),
        ([fndds_portion("1 medium", "50"), fndds_portion("1 cubic inch", "10")], None),
        ([fndds_portion("Guideline amount per cup of hot cereal", "61")], None),
        # Measures of the food before it's eaten, or with ice, aren't densities.
        ([fndds_portion("1 cup, dry, yields", "740")], None),
        ([fndds_portion("1 tablespoon dry yields 8 fl oz", "248")], None),
        ([fndds_portion("1 cup, unpopped, yields", "193")], None),
        ([fndds_portion("1 teaspoon, dry", "0.9")], None),
        ([fndds_portion("1 fl oz (with ice)", "23")], None),
        ([fndds_portion("1 cup ice", "110")], None),
        (
            [
                fndds_portion("1 fl oz (no ice)", "30"),
                fndds_portion("1 cup ice", "110"),
            ],
            240,
        ),
        ([fndds_portion("1 cup (yield after bone removed)", "184")], 184),
    ],
)
def test_fndds_density(rows, expected):
    assert IMPORTER.fndds_grams_per_cup(rows) == expected


def test_fndds_zero_volume_weight_fails():
    with pytest.raises(ValueError, match="Invalid FNDDS volume portion"):
        IMPORTER.fndds_grams_per_cup([fndds_portion("1 cup", "0")])


def test_fndds_records_have_no_names_or_pieces():
    data = IMPORTER.build_fndds_data(
        [
            {"fdc_id": "1", "description": "Guacamole, NFS"},
            {"fdc_id": "2", "description": "Milk, human"},
        ],
        [
            {"fdc_id": "1", "nutrient_id": "208", "amount": "155"},
            {"fdc_id": "1", "nutrient_id": "1008", "amount": "999"},
        ],
        [fndds_portion("1 cup", "240"), fndds_portion("1 medium", "50", row_id="2")],
    )
    assert "names" not in data
    assert data["foods"] == [
        {
            "fdc_id": 1,
            "description": "guacamole, nfs",
            "kcal_per_100g": 155.0,
            "grams_per_cup": 240.0,
            "portions": {},
            "default_portion": None,
        },
        {
            "fdc_id": 2,
            "description": "milk, human",
            "kcal_per_100g": None,
            "grams_per_cup": None,
            "portions": {},
            "default_portion": None,
        },
    ]


@pytest.mark.parametrize(
    ("foods", "nutrients", "rows", "match"),
    [
        ([], [], [fndds_portion("1 cup")], "nonempty"),
        (
            [{"fdc_id": "1", "description": "A"}, {"fdc_id": "1", "description": "B"}],
            [],
            [fndds_portion("1 cup")],
            "duplicate food",
        ),
        (
            [{"fdc_id": "1", "description": "A"}],
            [{"fdc_id": "9", "nutrient_id": "208", "amount": "1"}],
            [fndds_portion("1 cup")],
            "Unknown food in nutrient",
        ),
        (
            [{"fdc_id": "1", "description": "A"}],
            [],
            [fndds_portion("1 cup", food_id="9")],
            "Unknown food in portion",
        ),
    ],
)
def test_invalid_fndds_tables_fail(foods, nutrients, rows, match):
    with pytest.raises(ValueError, match=match):
        IMPORTER.build_fndds_data(foods, nutrients, rows)
