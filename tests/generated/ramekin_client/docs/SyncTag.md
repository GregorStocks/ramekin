# SyncTag


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**change_seq** | **int** | Sync stamp of the tag&#39;s last change. | 
**id** | **UUID** |  | 
**name** | **str** |  | 

## Example

```python
from ramekin_client.models.sync_tag import SyncTag

# TODO update the JSON string below
json = "{}"
# create an instance of SyncTag from a JSON string
sync_tag_instance = SyncTag.from_json(json)
# print the JSON string representation of the object
print(SyncTag.to_json())

# convert the object into a dict
sync_tag_dict = sync_tag_instance.to_dict()
# create an instance of SyncTag from a dict
sync_tag_from_dict = SyncTag.from_dict(sync_tag_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


