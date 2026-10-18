import uuid
from datetime import date, timedelta

import pytest

from conftest import make_ingredient
from ramekin_client.api import MealPlansApi, RecipesApi, SyncApi
from ramekin_client.exceptions import ApiException
from ramekin_client.models import (
    CreateRecipeRequest,
    MealPlanFields,
    MealPlanOp,
    OpKind,
    OpResult,
    SyncMealPlansRequest,
    UpdateRecipeRequest,
)

TODAY = date.today()


def make_recipe(client, title="Test Recipe"):
    return RecipesApi(client).create_recipe(
        CreateRecipeRequest(
            title=title,
            instructions="Cook it",
            ingredients=[make_ingredient(item="food")],
        )
    )


def create(recipe_id, meal_date=TODAY, meal_type="dinner", **fields):
    return MealPlanOp(
        op=OpKind.CREATE,
        id=str(uuid.uuid4()),
        fields=MealPlanFields(
            recipe_id=str(recipe_id), meal_date=meal_date, meal_type=meal_type, **fields
        ),
    )


def update(plan_id, **fields):
    return MealPlanOp(op=OpKind.UPDATE, id=plan_id, fields=MealPlanFields(**fields))


def delete(plan_id):
    return MealPlanOp(op=OpKind.DELETE, id=plan_id)


def sync(client, *ops, cursor=None):
    return SyncApi(client).sync_meal_plans(
        SyncMealPlansRequest(cursor=cursor, ops=list(ops))
    )


def plans(client):
    """Every live plan, by id, from a full sync."""
    return {plan.id: plan for plan in sync(client).changes}


def listed(client, start=TODAY, end=TODAY):
    return MealPlansApi(client).list_meal_plans(start_date=start, end_date=end)


def test_list_meal_plans_empty(authed_api_client):
    client, _ = authed_api_client
    assert listed(client).meal_plans == []


def test_list_meal_plans_requires_auth(unauthed_api_client):
    with pytest.raises(ApiException) as exc_info:
        MealPlansApi(unauthed_api_client).list_meal_plans()
    assert exc_info.value.status == 401


def test_sync_requires_auth(unauthed_api_client):
    with pytest.raises(ApiException) as exc_info:
        sync(unauthed_api_client)
    assert exc_info.value.status == 401


def test_create_meal_plan(authed_api_client):
    client, _ = authed_api_client
    recipe = make_recipe(client, "Detailed Recipe")
    op = create(recipe.id, notes="Make double batch")

    response = sync(client, op)

    assert response.results == [OpResult.APPLIED]
    [plan] = response.changes
    assert plan.id == op.id
    assert plan.recipe_id == recipe.id
    assert plan.recipe_title == "Detailed Recipe"
    assert plan.meal_date == TODAY
    assert plan.meal_type == "dinner"
    assert plan.notes == "Make double batch"
    [listed_plan] = listed(client).meal_plans
    assert listed_plan.id == op.id
    assert listed_plan.recipe_title == "Detailed Recipe"


def test_replaying_a_create_is_a_no_op(authed_api_client):
    client, _ = authed_api_client
    op = create(make_recipe(client).id)
    sync(client, op)

    response = sync(client, op)

    assert response.results == [OpResult.APPLIED]
    assert list(plans(client)) == [op.id]


def test_a_create_needs_recipe_date_and_meal(authed_api_client):
    client, _ = authed_api_client
    with pytest.raises(ApiException) as exc_info:
        sync(
            client,
            MealPlanOp(
                op=OpKind.CREATE,
                id=str(uuid.uuid4()),
                fields=MealPlanFields(meal_date=TODAY, meal_type="lunch"),
            ),
        )
    assert exc_info.value.status == 400


def test_create_with_a_missing_or_deleted_recipe(authed_api_client):
    client, _ = authed_api_client
    deleted = make_recipe(client)
    RecipesApi(client).delete_recipe(deleted.id)

    response = sync(client, create(uuid.uuid4()), create(deleted.id))

    assert response.results == [OpResult.RECIPE_MISSING, OpResult.RECIPE_MISSING]
    assert plans(client) == {}
    assert len(response.deleted) == 2


def test_planning_a_recipe_twice_for_one_meal_is_a_duplicate(authed_api_client):
    client, _ = authed_api_client
    recipe = make_recipe(client)

    first = create(recipe.id, meal_type="breakfast")
    second = create(recipe.id, meal_type="breakfast")

    response = sync(client, first, second)

    assert response.results == [OpResult.APPLIED, OpResult.DUPLICATE]
    assert len(plans(client)) == 1
    # The refused create's plan never existed, so it comes back as a
    # tombstone and the client drops it, even on a full resync.
    assert response.full_resync is True
    assert [plan.id for plan in response.changes] == [first.id]
    assert response.deleted == [second.id]


def test_different_recipes_share_a_meal(authed_api_client):
    client, _ = authed_api_client
    main = make_recipe(client, "Main Dish")
    side = make_recipe(client, "Side Dish")

    response = sync(client, create(main.id), create(side.id))

    assert response.results == [OpResult.APPLIED, OpResult.APPLIED]
    assert len(listed(client).meal_plans) == 2


def test_update_sets_only_the_fields_it_names(authed_api_client):
    client, _ = authed_api_client
    tomorrow = TODAY + timedelta(days=1)
    op = create(make_recipe(client).id, meal_type="lunch", notes="Keep me")
    sync(client, op)

    sync(client, update(op.id, meal_date=tomorrow))
    sync(client, update(op.id, meal_type="dinner"))

    plan = plans(client)[op.id]
    assert plan.meal_date == tomorrow
    assert plan.meal_type == "dinner"
    assert plan.notes == "Keep me"
    assert listed(client).meal_plans == []
    assert len(listed(client, tomorrow, tomorrow).meal_plans) == 1


def test_update_notes_and_clear_them(authed_api_client):
    client, _ = authed_api_client
    op = create(make_recipe(client).id)
    sync(client, op)

    sync(client, update(op.id, notes="Updated notes"))
    assert plans(client)[op.id].notes == "Updated notes"
    sync(client, update(op.id, notes=""))
    assert plans(client)[op.id].notes is None


def test_moving_onto_a_taken_slot_is_a_duplicate_and_returns_the_plan(
    authed_api_client,
):
    """The rejected plan comes back, even from before the cursor, so the
    client can retake the server's copy."""
    client, _ = authed_api_client
    recipe = make_recipe(client)
    lunch = create(recipe.id, meal_type="lunch")
    dinner = create(recipe.id, meal_type="dinner")
    cursor = sync(client, lunch, dinner).cursor

    response = sync(client, update(lunch.id, meal_type="dinner"), cursor=cursor)

    assert response.results == [OpResult.DUPLICATE]
    [plan] = response.changes
    assert plan.id == lunch.id
    assert plan.meal_type == "lunch"


def test_update_of_an_unknown_plan_is_gone(authed_api_client):
    client, _ = authed_api_client
    response = sync(client, update(str(uuid.uuid4()), notes="test"))
    assert response.results == [OpResult.GONE]


def test_delete_and_readd(authed_api_client):
    client, _ = authed_api_client
    recipe = make_recipe(client)
    first = create(recipe.id, meal_type="snack")
    cursor = sync(client, first).cursor

    response = sync(client, delete(first.id), cursor=cursor)
    assert response.results == [OpResult.APPLIED]
    assert first.id in response.deleted
    assert listed(client).meal_plans == []

    # The slot is free again.
    again = create(recipe.id, meal_type="snack")
    assert sync(client, again).results == [OpResult.APPLIED]
    assert list(plans(client)) == [again.id]


def test_delete_wins_over_a_later_update(authed_api_client):
    client, _ = authed_api_client
    op = create(make_recipe(client).id)
    sync(client, op)
    sync(client, delete(op.id))

    response = sync(client, update(op.id, notes="too late"), delete(op.id))

    assert response.results == [OpResult.GONE, OpResult.APPLIED]
    assert plans(client) == {}


def test_delete_of_an_unknown_plan_is_gone(authed_api_client):
    client, _ = authed_api_client
    assert sync(client, delete(str(uuid.uuid4()))).results == [OpResult.GONE]


def test_cross_user_isolation(authed_api_client, second_authed_api_client):
    client1, _ = authed_api_client
    client2, _ = second_authed_api_client
    recipe = make_recipe(client1, "Private Recipe")
    op = create(recipe.id, meal_type="lunch")
    sync(client1, op)

    assert listed(client2).meal_plans == []
    assert plans(client2) == {}
    response = sync(
        client2,
        update(op.id, notes="hacked"),
        delete(op.id),
        # Another user's recipe can't be planned.
        create(recipe.id),
        # Nor can another user's plan id be reused.
        MealPlanOp(
            op=OpKind.CREATE,
            id=op.id,
            fields=MealPlanFields(
                recipe_id=str(make_recipe(client2).id),
                meal_date=TODAY,
                meal_type="lunch",
            ),
        ),
    )
    assert response.results == [
        OpResult.GONE,
        OpResult.GONE,
        OpResult.RECIPE_MISSING,
        OpResult.GONE,
    ]
    assert plans(client1)[op.id].notes is None


def test_sync_from_cursor_returns_later_changes(authed_api_client):
    client, _ = authed_api_client
    recipe = make_recipe(client)
    changed = create(recipe.id, meal_type="lunch")
    untouched = create(recipe.id, meal_type="dinner")
    initial = sync(client, changed, untouched)

    sync(client, update(changed.id, notes="later"))
    response = sync(client, cursor=initial.cursor)

    assert [plan.id for plan in response.changes] == [changed.id]
    assert response.full_resync is False
    assert sync(client, cursor=response.cursor).changes == []


def test_a_new_recipe_version_resends_its_plans(authed_api_client):
    client, _ = authed_api_client
    recipes_api = RecipesApi(client)
    recipe = make_recipe(client, "Before")
    op = create(recipe.id)
    cursor = sync(client, op).cursor

    recipes_api.update_recipe(
        recipe.id,
        UpdateRecipeRequest(
            expected_version_id=recipes_api.get_recipe(recipe.id).version_id,
            title="After",
        ),
    )
    response = sync(client, cursor=cursor)

    [plan] = response.changes
    assert plan.id == op.id
    assert plan.recipe_title == "After"


def test_deleting_a_recipe_tombstones_its_plans(authed_api_client):
    client, _ = authed_api_client
    recipe = make_recipe(client, "Will Be Deleted")
    op = create(recipe.id)
    cursor = sync(client, op).cursor

    RecipesApi(client).delete_recipe(recipe.id)
    response = sync(client, cursor=cursor)

    assert response.changes == []
    assert response.deleted == [op.id]
    assert listed(client).meal_plans == []
    assert plans(client) == {}
    assert sync(client, update(op.id, notes="x")).results == [OpResult.GONE]


def test_full_resync_sends_no_tombstones(authed_api_client):
    client, _ = authed_api_client
    recipe = make_recipe(client)
    kept = create(recipe.id, meal_type="lunch")
    removed = create(recipe.id, meal_type="dinner")
    sync(client, kept, removed, delete(removed.id))

    current = sync(client)
    foreign = sync(client, cursor=current.cursor + 1_000_000_000)

    for response in [current, foreign]:
        assert [plan.id for plan in response.changes] == [kept.id]
        assert response.deleted == []
        assert response.full_resync is True


def test_list_meal_plans_date_range(authed_api_client):
    client, _ = authed_api_client
    recipe = make_recipe(client)
    yesterday = TODAY - timedelta(days=1)
    tomorrow = TODAY + timedelta(days=1)
    sync(
        client, *(create(recipe.id, meal_date=d) for d in [yesterday, TODAY, tomorrow])
    )

    assert len(listed(client).meal_plans) == 1
    assert len(listed(client, yesterday, TODAY).meal_plans) == 2
    assert len(listed(client, yesterday, tomorrow).meal_plans) == 3


def test_all_meal_types(authed_api_client):
    client, _ = authed_api_client
    recipe = make_recipe(client)
    meal_types = ["breakfast", "lunch", "dinner", "snack"]

    sync(client, *(create(recipe.id, meal_type=meal_type) for meal_type in meal_types))

    assert {plan.meal_type for plan in listed(client).meal_plans} == set(meal_types)
