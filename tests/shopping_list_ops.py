"""Helpers for writing the shopping list through `POST /api/sync/shopping-list`."""

import uuid

from ramekin_client.models import (
    OpKind,
    OpResult,
    ShoppingListFields,
    ShoppingListOp,
    SyncShoppingListRequest,
)


def create(item, **fields):
    return ShoppingListOp(
        op=OpKind.CREATE,
        id=str(uuid.uuid4()),
        fields=ShoppingListFields(item=item, **fields),
    )


def update(item_id, **fields):
    return ShoppingListOp(
        op=OpKind.UPDATE, id=item_id, fields=ShoppingListFields(**fields)
    )


def delete(item_id):
    return ShoppingListOp(op=OpKind.DELETE, id=item_id)


def sync(api, *ops, cursor=None):
    return api.sync_shopping_list(SyncShoppingListRequest(cursor=cursor, ops=list(ops)))


def add(api, *names):
    """Create items named `names` and return their ids."""
    ops = [create(name) for name in names]
    response = sync(api, *ops)
    assert response.results == [OpResult.APPLIED] * len(ops)
    return [op.id for op in ops]


def items(api):
    """Every live item, by id."""
    return {item.id: item for item in sync(api).changes}
