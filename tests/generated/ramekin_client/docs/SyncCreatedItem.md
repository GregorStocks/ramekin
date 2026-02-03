# SyncCreatedItem


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**client_id** | **UUID** |  | 
**server_id** | **UUID** |  | 
**version** | **int** |  | 

## Example

```python
from ramekin_client.models.sync_created_item import SyncCreatedItem

# TODO update the JSON string below
json = "{}"
# create an instance of SyncCreatedItem from a JSON string
sync_created_item_instance = SyncCreatedItem.from_json(json)
# print the JSON string representation of the object
print(SyncCreatedItem.to_json())

# convert the object into a dict
sync_created_item_dict = sync_created_item_instance.to_dict()
# create an instance of SyncCreatedItem from a dict
sync_created_item_from_dict = SyncCreatedItem.from_dict(sync_created_item_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


