# SyncUpdatedItem


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **UUID** |  | 
**success** | **bool** |  | 
**version** | **int** |  | 

## Example

```python
from ramekin_client.models.sync_updated_item import SyncUpdatedItem

# TODO update the JSON string below
json = "{}"
# create an instance of SyncUpdatedItem from a JSON string
sync_updated_item_instance = SyncUpdatedItem.from_json(json)
# print the JSON string representation of the object
print(SyncUpdatedItem.to_json())

# convert the object into a dict
sync_updated_item_dict = sync_updated_item_instance.to_dict()
# create an instance of SyncUpdatedItem from a dict
sync_updated_item_from_dict = SyncUpdatedItem.from_dict(sync_updated_item_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


