# SyncCreateItem

Request to create an item during sync (created offline)

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**amount** | **str** |  | [optional] 
**client_id** | **UUID** |  | 
**is_checked** | **bool** |  | 
**item** | **str** |  | 
**note** | **str** |  | [optional] 
**sort_order** | **int** |  | 
**source_recipe_id** | **UUID** |  | [optional] 
**source_recipe_title** | **str** |  | [optional] 

## Example

```python
from ramekin_client.models.sync_create_item import SyncCreateItem

# TODO update the JSON string below
json = "{}"
# create an instance of SyncCreateItem from a JSON string
sync_create_item_instance = SyncCreateItem.from_json(json)
# print the JSON string representation of the object
print(SyncCreateItem.to_json())

# convert the object into a dict
sync_create_item_dict = sync_create_item_instance.to_dict()
# create an instance of SyncCreateItem from a dict
sync_create_item_from_dict = SyncCreateItem.from_dict(sync_create_item_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


