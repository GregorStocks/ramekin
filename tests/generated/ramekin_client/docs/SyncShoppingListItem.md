# SyncShoppingListItem


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**amount** | **str** |  | [optional] 
**category** | **str** | Aisle category for grouping (override when set, otherwise computed). | 
**category_override** | **str** | User-selected category override; when set, it wins over computed category. | [optional] 
**change_seq** | **int** | Sync stamp of the row&#39;s last change. A cached copy with a higher one is newer than this row. | 
**computed_category** | **str** | Category computed from the item name before applying any override. | 
**id** | **UUID** |  | 
**is_checked** | **bool** |  | 
**item** | **str** |  | 
**note** | **str** |  | [optional] 
**sort_order** | **int** |  | 
**source_recipe_id** | **UUID** |  | [optional] 
**source_recipe_title** | **str** |  | [optional] 

## Example

```python
from ramekin_client.models.sync_shopping_list_item import SyncShoppingListItem

# TODO update the JSON string below
json = "{}"
# create an instance of SyncShoppingListItem from a JSON string
sync_shopping_list_item_instance = SyncShoppingListItem.from_json(json)
# print the JSON string representation of the object
print(SyncShoppingListItem.to_json())

# convert the object into a dict
sync_shopping_list_item_dict = sync_shopping_list_item_instance.to_dict()
# create an instance of SyncShoppingListItem from a dict
sync_shopping_list_item_from_dict = SyncShoppingListItem.from_dict(sync_shopping_list_item_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


