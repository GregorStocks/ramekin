"""Offline tests for merging classification decisions into the catalog."""

import importlib.util
from pathlib import Path

import pytest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts/apply-catalog-classification.py"
SPEC = importlib.util.spec_from_file_location("apply_classification", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
APPLY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(APPLY)

USDA = {
    "foods": [
        {"fdc_id": 1, "description": "garlic, raw"},
        {"fdc_id": 2, "description": "egg, yolk, raw, fresh"},
        {"fdc_id": 3, "description": "twin"},
        {"fdc_id": 4, "description": "twin"},
    ],
    "names": {"garlic": 1},
}
CURATED = {
    "entries": {"kosher salt": {"fdc_id": 1}},
    "aliases": {"egg yolk": "egg, yolk, raw, fresh"},
    "not_food": {"to serve": "A serving suggestion."},
    "rewrites": {},
}
CATEGORIES = {"Household", "Produce"}


def run(*decisions):
    return APPLY.apply(CURATED, USDA, CATEGORIES, list(decisions))


def test_valid_decisions_apply():
    updated, counts, rejections = run(
        {"name": "garlic cloves", "action": "alias", "target": "garlic"},
        {"name": "egg yolks", "action": "alias", "target": "egg, yolk, raw, fresh"},
        {"name": "kosher", "action": "alias", "target": "kosher salt"},
        {"name": "meat", "action": "ambiguous", "reason": "many meats"},
        {"name": "for the pan", "action": "not_food", "reason": "a note"},
        {"name": "baking twine", "action": "product", "category": "Household"},
        {
            "name": "garlic paste",
            "action": "entry",
            "fdc_id": 1,
            "grams_per_cup_value": 240,
            "grams_per_cup_source": "borrowed",
        },
        {"name": "moon dust", "action": "skip", "reason": "unknown"},
    )
    assert rejections == []
    assert updated["aliases"]["garlic cloves"] == "garlic"
    assert updated["aliases"]["meat"] is None
    assert updated["not_food"]["for the pan"] == "a note"
    assert updated["entries"]["baking twine"] == {
        "kind": "product",
        "category": "Household",
    }
    assert updated["entries"]["garlic paste"]["grams_per_cup"]["value"] == 240.0
    assert "moon dust" not in updated["aliases"]
    assert counts["alias"] == 3 and counts["skip"] == 1
    assert "garlic cloves" not in CURATED["aliases"], "inputs are not mutated"


@pytest.mark.parametrize(
    ("decision", "why"),
    [
        ({"name": "Garlic Cloves", "action": "skip"}, "not normalized"),
        ({"name": "x", "action": "guess"}, "unknown action"),
        ({"name": "garlic", "action": "alias", "target": "garlic"}, "already"),
        ({"name": "to serve", "action": "ambiguous"}, "already"),
        ({"name": "x", "action": "alias", "target": "egg yolk"}, "is not an entry"),
        ({"name": "x", "action": "alias", "target": "twin"}, "is not an entry"),
        ({"name": "x", "action": "product", "category": "Aisle 9"}, "category"),
        ({"name": "x", "action": "entry", "fdc_id": 99}, "fdc_id"),
        (
            {"name": "x", "action": "entry", "fdc_id": 1, "grams_per_cup_value": 5},
            "source",
        ),
        (
            {
                "name": "x",
                "action": "entry",
                "fdc_id": 1,
                "grams_per_cup_value": True,
                "grams_per_cup_source": "s",
            },
            "positive value",
        ),
        ({"name": "x", "action": "not_food"}, "reason"),
    ],
)
def test_invalid_decisions_are_rejected(decision, why):
    _, _, rejections = run(decision)
    assert len(rejections) == 1 and why in rejections[0], rejections


def test_duplicate_names_are_rejected():
    _, _, rejections = run(
        {"name": "x", "action": "skip"}, {"name": "x", "action": "skip"}
    )
    assert rejections == ["'x': duplicate decision"]


def test_categories_come_from_the_categorizer():
    source = (
        Path(__file__).resolve().parents[1]
        / "ramekin-core/src/ingredient_categorizer.rs"
    ).read_text()
    categories = APPLY.shopping_categories(source)
    assert {"Produce", "Household", "Other"} <= categories
    assert len(categories) == 19


def test_aliases_can_target_entries_from_the_same_batch():
    updated, _, rejections = run(
        {"name": "garlic paste spread", "action": "alias", "target": "garlic paste"},
        {"name": "garlic paste", "action": "entry", "fdc_id": 1},
    )
    assert rejections == []
    assert updated["aliases"]["garlic paste spread"] == "garlic paste"
