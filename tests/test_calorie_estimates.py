import pytest
import requests

from conftest import make_ingredient
from ramekin_client.api import RecipesApi
from ramekin_client.models import CreateRecipeRequest, UpdateRecipeRequest


def estimate(server_url, client, ingredients, scale=1, servings=None):
    return requests.post(
        f"{server_url}/api/recipes/estimate-calories",
        headers={"Authorization": f"Bearer {client.configuration.access_token}"},
        json={
            "ingredients": [ingredient.to_dict() for ingredient in ingredients],
            "scale": scale,
            "servings": servings,
        },
        timeout=10,
    )


def test_estimate_requires_auth(server_url):
    response = requests.post(
        f"{server_url}/api/recipes/estimate-calories",
        json={"ingredients": [], "scale": 1},
        timeout=10,
    )
    assert response.status_code == 401


@pytest.mark.parametrize("scale", [0, -1, 1000001])
def test_estimate_invalid_scale(server_url, authed_api_client, scale):
    client, _ = authed_api_client
    response = estimate(server_url, client, [], scale=scale)
    assert response.status_code == 400


def test_estimate_ranges_partial_unknown_and_empty(server_url, authed_api_client):
    client, _ = authed_api_client
    sugar = make_ingredient("granulated sugar", "100-200", "g")
    yogurt = make_ingredient("yogurt", "1", "cup")
    response = estimate(server_url, client, [sugar, yogurt], scale=2, servings="4")
    assert response.status_code == 200
    result = response.json()
    assert result["known_calories"] == {"min": 774, "max": 1548}
    assert result["per_serving_calories"] == {"min": 96.75, "max": 193.5}
    assert result["status"] == "partial"
    assert result["headline"] == "At least ~96 kcal per serving"
    assert result["secondary"] == "At least ~770 kcal for the whole recipe"
    assert result["not_counted"] == ["yogurt"]
    assert result["lines"] == [
        {
            "index": 0,
            "item": "granulated sugar",
            "calories": {"min": 774, "max": 1548},
            "text": "~770–1,550 kcal",
        },
        {
            "index": 1,
            "item": "yogurt",
            "calories": None,
            "text": "Could be several foods",
        },
    ]
    assert (
        result
        == estimate(server_url, client, [sugar, yogurt], scale=2, servings="4").json()
    )
    for ingredients in [[yogurt], []]:
        unknown = estimate(server_url, client, ingredients, servings="4").json()
        assert unknown["known_calories"] is None
        assert unknown["per_serving_calories"] is None


def test_estimate_negligible_compound_and_skipped_lines(server_url, authed_api_client):
    client, _ = authed_api_client
    ingredients = [
        make_ingredient("granulated sugar", "100", "g"),
        make_ingredient("salt and freshly ground black pepper", note="to taste"),
        make_ingredient("For the topping:"),
        make_ingredient("parchment paper"),
        make_ingredient("olive oil and balsamic vinegar", "2", "tbsp"),
    ]
    response = estimate(server_url, client, ingredients)
    assert response.status_code == 200
    result = response.json()
    # Seasoning to taste counts as zero; the header and parchment are skipped.
    assert result["known_calories"] == {"min": 387, "max": 387}
    assert [line["text"] for line in result["lines"]] == [
        "~390 kcal",
        "Negligible",
        "Not a food",
        "Not a food",
        "Several foods share one amount",
    ]
    assert result["status"] == "partial"
    assert result["not_counted"] == ["olive oil and balsamic vinegar"]


@pytest.mark.parametrize(
    "amount,unit,expected",
    [
        ("1-1/2", "g", 5.805),
        ("1,5", "g", 5.805),
        (",5", "g", 1.935),
        ("0,125", "g", 0.48375),
        ("1 tablespoon plus 1 teaspoon", None, 64.5),
        ("1/4 plus 1/8", "cup", 290.25),
    ],
)
def test_estimate_preserved_parser_amounts(
    server_url, authed_api_client, amount, unit, expected
):
    client, _ = authed_api_client
    response = estimate(
        server_url,
        client,
        [make_ingredient("granulated sugar", amount, unit)],
        servings="Servings: 8",
    )
    assert response.status_code == 200
    result = response.json()
    assert result["status"] == "complete"
    assert result["known_calories"]["min"] == pytest.approx(expected)
    assert result["known_calories"]["max"] == pytest.approx(expected)
    assert result["per_serving_calories"]["min"] == pytest.approx(expected / 8)


def test_estimate_uses_displayed_version_and_edited_quantities(
    server_url, authed_api_client
):
    client, _ = authed_api_client
    api = RecipesApi(client)
    created = api.create_recipe(
        CreateRecipeRequest(
            title="Calorie version test",
            instructions="Mix",
            ingredients=[make_ingredient("granulated sugar", "100", "g")],
            servings="2",
            nutritional_info="Imported nutrition text",
        )
    )
    original = api.get_recipe(created.id)
    api.update_recipe(
        created.id,
        UpdateRecipeRequest(
            expected_version_id=original.version_id,
            ingredients=[make_ingredient("granulated sugar", "200", "g")],
        ),
    )
    current = api.get_recipe(created.id)
    historical = api.get_recipe(created.id, version_id=original.version_id)
    for recipe, expected in [(current, 774), (historical, 387)]:
        result = estimate(
            server_url, client, recipe.ingredients, servings=recipe.servings
        ).json()
        assert result["known_calories"] == {"min": expected, "max": expected}
        assert result["status"] == "complete"
        assert result["headline"].endswith("kcal per serving")
        assert result["not_counted"] == []
        assert recipe.nutritional_info == "Imported nutrition text"
    # The estimate is stateless, including a changed quantity not yet saved.
    result = estimate(
        server_url, client, [make_ingredient("granulated sugar", "1", "cup")]
    ).json()
    assert result["known_calories"] == {"min": 774, "max": 774}


def test_estimate_status_follows_uncounted_ingredients(server_url, authed_api_client):
    client, _ = authed_api_client
    sugar = make_ingredient("granulated sugar", "100", "g")
    unknown = [make_ingredient(f"moon dust {i}", "1", "cup") for i in range(4)]
    for count, status in [(0, "complete"), (3, "partial"), (4, "insufficient")]:
        result = estimate(server_url, client, [sugar, *unknown[:count]]).json()
        assert result["status"] == status, count
    insufficient = estimate(server_url, client, [sugar, *unknown]).json()
    assert insufficient["headline"] == "Not enough ingredient data to estimate calories"
    assert insufficient["secondary"] == "4 ingredients couldn't be counted."
    assert insufficient["not_counted"] == []
    empty = estimate(server_url, client, []).json()
    assert empty["status"] == "empty"
    assert empty["headline"] == "No ingredients to estimate"
