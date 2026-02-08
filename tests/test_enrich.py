from conftest import make_ingredient
from ramekin_client.api import EnrichApi
from ramekin_client.models import RecipeContent


def test_enrich_adds_gram_conversion_for_volume_units(authed_api_client):
    """Enrich adds gram alternatives for known volume-unit ingredients."""
    client, user_id = authed_api_client
    enrich_api = EnrichApi(client)

    content = RecipeContent(
        title="Test Recipe",
        instructions="Mix and serve.",
        ingredients=[
            make_ingredient(item="sugar", amount="2", unit="tbsp"),
        ],
    )

    result = enrich_api.enrich_recipe(content)

    sugar = result.ingredients[0]
    assert sugar.item == "sugar"
    assert len(sugar.measurements) == 2
    assert sugar.measurements[0].amount == "2"
    assert sugar.measurements[0].unit == "tbsp"
    assert sugar.measurements[1].unit == "g"
    assert sugar.measurements[1].amount == "25"  # 2 tbsp sugar = 25g


def test_enrich_adds_gram_conversion_for_mayo(authed_api_client):
    """Test that mayo (newly added alias) gets gram conversion."""
    client, user_id = authed_api_client
    enrich_api = EnrichApi(client)

    content = RecipeContent(
        title="Egg Salad",
        instructions="Mix everything.",
        ingredients=[
            make_ingredient(item="mayo", amount="3", unit="tbsp"),
            make_ingredient(item="mustard", amount="1.5", unit="tbsp"),
        ],
    )

    result = enrich_api.enrich_recipe(content)

    mayo = result.ingredients[0]
    assert mayo.item == "mayo"
    assert len(mayo.measurements) == 2
    assert mayo.measurements[1].unit == "g"

    mustard = result.ingredients[1]
    assert mustard.item == "mustard"
    assert len(mustard.measurements) == 2
    assert mustard.measurements[1].unit == "g"


def test_enrich_preserves_existing_weight_measurements(authed_api_client):
    """Test that ingredients already having weight units are not double-enriched."""
    client, user_id = authed_api_client
    enrich_api = EnrichApi(client)

    content = RecipeContent(
        title="Test Recipe",
        instructions="Cook it.",
        ingredients=[
            make_ingredient(item="chicken", amount="8", unit="oz"),
        ],
    )

    result = enrich_api.enrich_recipe(content)

    chicken = result.ingredients[0]
    # oz → g conversion should be added
    assert len(chicken.measurements) == 2
    assert chicken.measurements[1].unit == "g"
    assert chicken.measurements[1].amount == "227"  # 8 oz = 226.8g → 227g


def test_enrich_handles_unknown_ingredients(authed_api_client):
    """Test that unknown ingredients are returned unchanged (no crash)."""
    client, user_id = authed_api_client
    enrich_api = EnrichApi(client)

    content = RecipeContent(
        title="Test Recipe",
        instructions="Mix.",
        ingredients=[
            make_ingredient(item="unicorn tears", amount="1", unit="cup"),
        ],
    )

    result = enrich_api.enrich_recipe(content)

    tears = result.ingredients[0]
    assert tears.item == "unicorn tears"
    assert len(tears.measurements) == 1  # no gram alternative added


def test_enrich_handles_no_unit_ingredients(authed_api_client):
    """Test that count-based ingredients (no unit) are returned unchanged."""
    client, user_id = authed_api_client
    enrich_api = EnrichApi(client)

    content = RecipeContent(
        title="Test Recipe",
        instructions="Boil eggs.",
        ingredients=[
            make_ingredient(item="eggs", amount="6"),
        ],
    )

    result = enrich_api.enrich_recipe(content)

    eggs = result.ingredients[0]
    assert eggs.item == "eggs"
    assert len(eggs.measurements) == 1  # no conversion for count-based
