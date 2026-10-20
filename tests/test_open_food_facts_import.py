"""Offline tests for the Open Food Facts shopping-aisle import."""

import gzip
import hashlib
import importlib.util
import json
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/import-open-food-facts.py"
SPEC = importlib.util.spec_from_file_location("open_food_facts_import", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
IMPORTER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(IMPORTER)

CATEGORIES = {"Snacks", "Frozen", "Produce", "Other"}
TAXONOMY = {
    "en:snacks": {"name": {"en": "Snacks"}},
    "en:breakfast-cereals": {
        "name": {"en": "Breakfast cereals"},
        "parents": ["en:snacks"],
    },
    "en:oat-rings": {
        "name": {"en": "Oat rings"},
        "synonyms": {"en": ["Oat rings", "Cheerios"]},
        "parents": ["en:breakfast-cereals"],
    },
    "en:frozen-foods": {"name": {"en": "Frozen foods"}},
    "en:vegetables": {"name": {"en": "Vegetables"}},
    "en:frozen-vegetables": {
        "name": {"en": "Frozen vegetables"},
        "parents": ["en:vegetables", "en:frozen-foods"],
    },
    "en:peas": {"name": {"en": "Peas"}, "parents": ["en:vegetables"]},
    "en:snack-peas": {"name": {"en": "Peas"}, "parents": ["en:snacks"]},
    "en:unmapped": {"name": {"en": "Unmapped"}},
}
PAIRS = [
    ("en:frozen-foods", "Frozen"),
    ("en:snacks", "Snacks"),
    ("en:vegetables", "Produce"),
]


def aisles():
    return IMPORTER.Aisles(TAXONOMY, PAIRS)


def row(name="", brands="", category="en:snacks", countries="en:united-states"):
    return {
        "product_name": name,
        "brands_en": brands,
        "main_category": category,
        "countries_tags": countries,
    }


def test_name_key():
    assert IMPORTER.name_key("Cheez-It") == "cheez it"
    assert IMPORTER.name_key("S’mores") == "smores"
    assert IMPORTER.name_key("  Ben & Jerry's_Ice ") == "ben jerrys ice"


def test_nearest_listed_ancestor_and_tie_order():
    found = aisles()
    assert found.of("en:oat-rings") == "Snacks"
    # Both parents are listed and equally near: the earlier entry wins.
    assert found.of("en:frozen-vegetables") == "Frozen"
    assert found.of("en:unmapped") is None


def test_taxonomy_names_use_synonyms_and_drop_conflicts():
    names = IMPORTER.taxonomy_names(TAXONOMY, aisles())
    assert names["cheerios"] == "Snacks"
    assert names["frozen vegetables"] == "Frozen"
    # "Peas" names a Produce and a Snacks category.
    assert "peas" not in names
    assert "unmapped" not in names


def test_product_votes_need_enough_agreeing_us_products():
    rows = [
        row("Cheez-It", "Sunshine"),
        row("Cheez-It", "Sunshine"),
        row("Cheez-It", "Sunshine"),
        row("Cheez-It", countries="en:france"),
        row("Rare chips"),
        row("Rare chips"),
        row("Split", category="en:snacks"),
        row("Split", category="en:snacks"),
        row("Split", category="en:vegetables"),
        row("Split", category="en:vegetables"),
        row("Unknown", category="en:unmapped"),
    ]
    names = IMPORTER.build_names(TAXONOMY, PAIRS, rows)
    assert names["cheez it"] == "Snacks"
    assert names["sunshine"] == "Snacks"
    # Two products are too few, and a 50/50 split agrees on nothing.
    assert "rare chips" not in names
    assert "split" not in names
    assert "unknown" not in names


def test_taxonomy_wins_over_products():
    rows = [row("Cheerios", category="en:vegetables")] * 3
    assert IMPORTER.build_names(TAXONOMY, PAIRS, rows)["cheerios"] == "Snacks"


@pytest.mark.parametrize(
    ("aisles", "match"),
    [
        ([["en:snacks", "Snacks"], ["en:snacks", "Other"]], "listed twice"),
        ([["en:nope", "Snacks"]], "not an Open Food Facts category"),
        ([["en:snacks", "Aisle 9"]], "unknown aisle"),
    ],
)
def test_invalid_mapping_fails(aisles, match):
    with pytest.raises(ValueError, match=match):
        IMPORTER.load_mapping({"aisles": aisles}, TAXONOMY, CATEGORIES)


def test_mapping_aisles_are_categorizer_categories():
    mapping = json.loads(IMPORTER.MAPPING.read_text(encoding="utf-8"))
    categories = IMPORTER.shopping_categories(IMPORTER.CATEGORIZER.read_text())
    assert {aisle for _, aisle in mapping["aisles"]} <= categories


def test_import_from_cache_records_checksums(tmp_path, monkeypatch):
    taxonomy = tmp_path / "categories.json"
    taxonomy.write_text(json.dumps(TAXONOMY), encoding="utf-8")
    products = tmp_path / "products.csv.gz"
    header = "product_name\tbrands_en\tmain_category\tcountries_tags\n"
    lines = "Pretzel thins\tSnyder's\ten:snacks\ten:united-states\n" * 3
    with gzip.open(products, "wt", encoding="utf-8") as stream:
        stream.write(header + lines)
    mapping = tmp_path / "aisles.json"
    mapping.write_text(json.dumps({"aisles": PAIRS}), encoding="utf-8")
    categorizer = tmp_path / "categorizer.rs"
    categorizer.write_text(
        'pub const CATEGORIES: [&str; 4] = ["Snacks", "Frozen", "Produce", "Other"];'
    )
    output = tmp_path / "open_food_facts.json"
    monkeypatch.setattr(IMPORTER, "TAXONOMY_CACHE", taxonomy)
    monkeypatch.setattr(IMPORTER, "PRODUCTS_CACHE", products)
    monkeypatch.setattr(IMPORTER, "MAPPING", mapping)
    monkeypatch.setattr(IMPORTER, "CATEGORIZER", categorizer)
    monkeypatch.setattr(IMPORTER, "OUTPUT", output)
    monkeypatch.setattr(IMPORTER, "ROOT", tmp_path)

    def no_download(*_args, **_kwargs):
        raise AssertionError("cached inputs must not be downloaded")

    monkeypatch.setattr(IMPORTER.urllib.request, "urlopen", no_download)
    IMPORTER.main([])
    data = json.loads(output.read_text(encoding="utf-8"))
    assert data["names"]["pretzel thins"] == "Snacks"
    assert data["names"]["snyders"] == "Snacks"
    assert data["names"]["cheerios"] == "Snacks"
    assert (
        data["sources"][IMPORTER.PRODUCTS_SOURCE]
        == hashlib.sha256(products.read_bytes()).hexdigest()
    )
    assert "Open Database License" in data["attribution"]
