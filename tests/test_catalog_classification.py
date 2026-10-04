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
    "entries": {
        "kosher salt": {"fdc_id": 1},
        "foil": {"kind": "product", "category": "Household"},
    },
    "aliases": {"egg yolk": "egg, yolk, raw, fresh", "cheese": None},
    "not_food": {"to serve": "A serving suggestion."},
    "rewrites": {},
}
CATEGORIES = {"Household", "Produce"}
FNDDS = {"foods": [{"fdc_id": 50, "description": "guacamole, nfs"}]}


def run(*decisions):
    return APPLY.apply(CURATED, USDA, CATEGORIES, list(decisions), FNDDS)


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


def test_entries_may_link_fndds_foods():
    updated, _, rejections = run({"name": "guacamole", "action": "entry", "fdc_id": 50})
    assert rejections == []
    assert updated["entries"]["guacamole"] == {"fdc_id": 50}


def test_hand_curated_foods_need_cited_calories():
    food = {
        "name": "garam masala",
        "action": "food",
        "kcal_per_100g_value": 300,
        "kcal_per_100g_source": "USDA FDC Branded 2028654 label",
        "kcal_per_100g_url": "https://fdc.nal.usda.gov/food-details/2028654/nutrients",
        "grams_per_cup_value": 160,
        "grams_per_cup_source": "label: 1 Tbsp = 10 g",
        "trace_ok": True,
    }
    updated, counts, rejections = run(food)
    assert rejections == [] and counts["food"] == 1
    assert updated["entries"]["garam masala"] == {
        "kcal_per_100g": {
            "value": 300.0,
            "source": "USDA FDC Branded 2028654 label",
            "url": "https://fdc.nal.usda.gov/food-details/2028654/nutrients",
        },
        "grams_per_cup": {"value": 160.0, "source": "label: 1 Tbsp = 10 g"},
        "trace_ok": True,
    }
    no_url = {k: v for k, v in food.items() if k != "kcal_per_100g_url"}
    _, _, rejections = run(no_url)
    assert rejections and "url" in rejections[0]
    no_kcal = {"name": "x", "action": "food"}
    _, _, rejections = run(no_kcal)
    assert rejections and "kcal_per_100g" in rejections[0]
    bad = {**food, "kcal_per_100g_value": -1}
    _, _, rejections = run(bad)
    assert rejections and "positive value" in rejections[0]
    for url in [7, "", "fdc.nal.usda.gov/food-details/1", True]:
        _, _, rejections = run({**food, "kcal_per_100g_url": url})
        assert rejections, url
    _, _, rejections = run({**food, "grams_per_cup_source": 3})
    assert rejections and "source" in rejections[0]
    for value in [float("inf"), float("nan")]:
        _, _, rejections = run({**food, "kcal_per_100g_value": value})
        assert rejections and "positive value" in rejections[0], value
    for trace_ok in ["false", 1, None]:
        _, _, rejections = run({**food, "trace_ok": trace_ok})
        assert rejections and "trace_ok" in rejections[0], trace_ok


def test_trace_entries_carry_only_trace_ok():
    updated, counts, rejections = run(
        {"name": "sumac", "action": "trace", "reason": "No citable label."},
        {"name": "ground sumac", "action": "alias", "target": "sumac"},
    )
    assert rejections == [] and counts["trace"] == 1
    assert updated["entries"]["sumac"] == {"trace_ok": True}
    assert updated["aliases"]["ground sumac"] == "sumac"
    _, _, rejections = run({"name": "sumac", "action": "trace"})
    assert rejections and "reason" in rejections[0]


def test_categories_key_foods_and_ambiguous_names():
    updated, counts, rejections = run(
        {"name": "kosher salt", "action": "category", "category": "Produce"},
        {"name": "garlic", "action": "category", "category": "Produce"},
        {"name": "egg, yolk, raw, fresh", "action": "category", "category": "Produce"},
        {"name": "twin", "action": "category", "category": "Produce"},
        {"name": "cheese", "action": "category", "category": "Produce"},
    )
    assert rejections == [] and counts["category"] == 5
    assert updated["categories"] == {
        "kosher salt": "Produce",
        "garlic": "Produce",
        "egg, yolk, raw, fresh": "Produce",
        "twin": "Produce",
        "cheese": "Produce",
    }
    assert "categories" not in CURATED, "inputs are not mutated"


@pytest.mark.parametrize(
    ("decision", "why"),
    [
        ({"name": "garlic", "action": "category", "category": "Aisle 9"}, "category"),
        (
            {"name": "egg yolk", "action": "category", "category": "Produce"},
            "not a food",
        ),
        ({"name": "foil", "action": "category", "category": "Household"}, "not a food"),
        (
            {"name": "to serve", "action": "category", "category": "Produce"},
            "not a food",
        ),
        (
            {"name": "moon dust", "action": "category", "category": "Produce"},
            "not a food",
        ),
    ],
)
def test_invalid_categories_are_rejected(decision, why):
    _, _, rejections = run(decision)
    assert len(rejections) == 1 and why in rejections[0], rejections


def test_two_spellings_of_one_entry_are_categorized_once():
    _, _, rejections = run(
        {"name": "garlic", "action": "category", "category": "Produce"},
        {"name": "garlic, raw", "action": "category", "category": "Produce"},
    )
    assert rejections == ["'garlic, raw': already categorized"]
    curated = {**CURATED, "categories": {"garlic, raw": "Produce"}}
    _, _, rejections = APPLY.apply(
        curated,
        USDA,
        CATEGORIES,
        [{"name": "garlic", "action": "category", "category": "Produce"}],
        FNDDS,
    )
    assert rejections == ["'garlic': already categorized"]


def test_categorized_names_are_not_recategorized():
    curated = {**CURATED, "categories": {"garlic": "Produce"}}
    _, _, rejections = APPLY.apply(
        curated,
        USDA,
        CATEGORIES,
        [{"name": "garlic", "action": "category", "category": "Produce"}],
        FNDDS,
    )
    assert rejections == ["'garlic': already categorized"]
