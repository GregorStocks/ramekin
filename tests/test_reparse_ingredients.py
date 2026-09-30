from conftest import make_ingredient
from ramekin_client.api import RecipesApi
from ramekin_client.models import CreateRecipeRequest


def create_recipe(api, title, ingredients, tags=None):
    return api.create_recipe(
        CreateRecipeRequest(
            title=title,
            instructions="Cook.",
            ingredients=ingredients,
            tags=tags or [],
        )
    ).id


def test_reparse_moves_junk_out_of_items_and_saves_a_version(authed_api_client):
    client, _ = authed_api_client
    api = RecipesApi(client)
    junk = create_recipe(
        api,
        "Junk items",
        [
            make_ingredient("chickpeas, drained, rinsed", "2", "cup", "from 1 can"),
            make_ingredient("about 7 cloves garlic, minced"),
            make_ingredient("85% lean ground beef", "1", "lb"),
        ],
        tags=["weeknight"],
    )
    clean = create_recipe(api, "Clean items", [make_ingredient("garlic", "2", "clove")])
    before = api.get_recipe(junk)

    result = api.reparse_all_ingredients()
    assert result.recipes_checked == 2
    assert result.recipes_updated == 1
    assert result.ingredients_changed == 2

    after = api.get_recipe(junk)
    assert after.version_id != before.version_id
    assert after.version_source == "reparse"
    assert after.tags == ["weeknight"]
    chickpeas, garlic, beef = after.ingredients
    assert chickpeas.item == "chickpeas"
    assert chickpeas.note == "drained, rinsed, from 1 can"
    assert chickpeas.measurements[0].amount == "2"
    assert garlic.item == "garlic"
    assert garlic.measurements[0].amount == "7"
    assert garlic.measurements[0].unit == "clove"
    assert beef.item == "85% lean ground beef"
    # The earlier version is kept, not replaced.
    versions = api.list_versions(junk).versions
    assert [v.version_source for v in versions][:2] == ["reparse", "user"]
    assert api.get_recipe(clean).version_source == "user"

    # Nothing left to change on a second run.
    again = api.reparse_all_ingredients()
    assert again.recipes_updated == 0
    assert again.ingredients_changed == 0


def test_reparse_only_touches_the_callers_recipes(
    authed_api_client, second_authed_api_client
):
    client, _ = authed_api_client
    other_client, _ = second_authed_api_client
    other = RecipesApi(other_client)
    theirs = create_recipe(
        other, "Theirs", [make_ingredient("chickpeas, drained", "1", "cup")]
    )
    result = RecipesApi(client).reparse_all_ingredients()
    assert result.recipes_checked == 0
    assert other.get_recipe(theirs).version_source == "user"
