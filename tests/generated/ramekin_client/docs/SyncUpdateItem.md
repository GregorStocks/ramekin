# SyncUpdateItem

Request to update an item during sync (modified offline)

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**amount** | **str** |  | [optional] 
**expected_version** | **int** | Expected version for optimistic locking | 
**id** | **UUID** |  | 
**is_checked** | **bool** |  | [optional] 
**item** | **str** |  | [optional] 
**note** | **str** |  | [optional] 
**sort_order** | **int** |  | [optional] 

## Example

```python
from ramekin_client.models.sync_update_item import SyncUpdateItem

# TODO update the JSON string below
json = "{}"
# create an instance of SyncUpdateItem from a JSON string
sync_update_item_instance = SyncUpdateItem.from_json(json)
# print the JSON string representation of the object
print(SyncUpdateItem.to_json())

# convert the object into a dict
sync_update_item_dict = sync_update_item_instance.to_dict()
# create an instance of SyncUpdateItem from a dict
sync_update_item_from_dict = SyncUpdateItem.from_dict(sync_update_item_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


