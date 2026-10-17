# SyncTagsResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**changes** | [**List[SyncTag]**](SyncTag.md) | Live tags created, renamed or revived at or after &#x60;cursor&#x60;; every live tag on a full resync. | 
**cursor** | **int** | Pass as &#x60;cursor&#x60; on the next sync. | 
**deleted** | **List[UUID]** | Tag IDs deleted at or after &#x60;cursor&#x60;. Empty on a full resync. | 
**full_resync** | **bool** | True when &#x60;changes&#x60; holds every live tag: drop cached tags it omits. | 

## Example

```python
from ramekin_client.models.sync_tags_response import SyncTagsResponse

# TODO update the JSON string below
json = "{}"
# create an instance of SyncTagsResponse from a JSON string
sync_tags_response_instance = SyncTagsResponse.from_json(json)
# print the JSON string representation of the object
print(SyncTagsResponse.to_json())

# convert the object into a dict
sync_tags_response_dict = sync_tags_response_instance.to_dict()
# create an instance of SyncTagsResponse from a dict
sync_tags_response_from_dict = SyncTagsResponse.from_dict(sync_tags_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


