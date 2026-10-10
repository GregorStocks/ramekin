# SyncResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**category_order** | **List[str]** | Canonical category display order for grouping items; every item&#39;s &#x60;category&#x60; is guaranteed to appear in this list. | 
**created** | [**List[SyncCreatedItem]**](SyncCreatedItem.md) | Items that were created (maps client_id to server_id) | 
**cursor** | **int** | Snapshot watermark to pass as &#x60;cursor&#x60; on the next sync. Changes may be redelivered across syncs, but none can be skipped. | 
**deleted** | **List[UUID]** | IDs of items that were deleted | 
**server_changes** | [**List[SyncServerChange]**](SyncServerChange.md) | Server-side changes since the request&#39;s cursor | 
**sync_timestamp** | **datetime** | Server time when this sync started. Deprecated as a cursor: pass &#x60;cursor&#x60; instead. | 
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


