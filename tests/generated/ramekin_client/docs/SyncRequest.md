# SyncRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**creates** | [**List[SyncCreateItem]**](SyncCreateItem.md) | Items created offline | [optional] 
**cursor** | **int** | &#x60;cursor&#x60; from the previous sync&#39;s response. Server returns changes at or after it; without one, it returns every item. | [optional] 
**deletes** | **List[UUID]** | IDs of items deleted offline | [optional] 
**updates** | [**List[SyncUpdateItem]**](SyncUpdateItem.md) | Items updated offline | [optional] 

## Example

```python
from ramekin_client.models.sync_request import SyncRequest

# TODO update the JSON string below
json = "{}"
# create an instance of SyncRequest from a JSON string
sync_request_instance = SyncRequest.from_json(json)
# print the JSON string representation of the object
print(SyncRequest.to_json())

# convert the object into a dict
sync_request_dict = sync_request_instance.to_dict()
# create an instance of SyncRequest from a dict
sync_request_from_dict = SyncRequest.from_dict(sync_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


