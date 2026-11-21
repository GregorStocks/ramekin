# SyncPhoto

The stored facts about one photo that search filters on.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**file_size** | **int** | Size of the stored file in bytes. | [optional] 
**height** | **int** |  | [optional] 
**width** | **int** |  | [optional] 

## Example

```python
from ramekin_client.models.sync_photo import SyncPhoto

# TODO update the JSON string below
json = "{}"
# create an instance of SyncPhoto from a JSON string
sync_photo_instance = SyncPhoto.from_json(json)
# print the JSON string representation of the object
print(SyncPhoto.to_json())

# convert the object into a dict
sync_photo_dict = sync_photo_instance.to_dict()
# create an instance of SyncPhoto from a dict
sync_photo_from_dict = SyncPhoto.from_dict(sync_photo_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


