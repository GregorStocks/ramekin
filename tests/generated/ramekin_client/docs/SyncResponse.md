# SyncResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**created** | [**List[SyncCreatedItem]**](SyncCreatedItem.md) | Items that were created (maps client_id to server_id) | 
**deleted** | **List[UUID]** | IDs of items that were deleted | 
**server_changes** | [**List[SyncServerChange]**](SyncServerChange.md) | Server-side changes since last_sync_at | 
**sync_timestamp** | **datetime** | New sync timestamp to use for next sync | 
**updated** | [**List[SyncUpdatedItem]**](SyncUpdatedItem.md) | Items that were updated (with success status) | 

## Example

```python
from ramekin_client.models.sync_response import SyncResponse

# TODO update the JSON string below
json = "{}"
# create an instance of SyncResponse from a JSON string
sync_response_instance = SyncResponse.from_json(json)
# print the JSON string representation of the object
print(SyncResponse.to_json())

# convert the object into a dict
sync_response_dict = sync_response_instance.to_dict()
# create an instance of SyncResponse from a dict
sync_response_from_dict = SyncResponse.from_dict(sync_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


