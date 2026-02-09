# ImportFromPhotosRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**photo_ids** | **List[UUID]** | Photo IDs that have already been uploaded via POST /api/photos | 

## Example

```python
from ramekin_client.models.import_from_photos_request import ImportFromPhotosRequest

# TODO update the JSON string below
json = "{}"
# create an instance of ImportFromPhotosRequest from a JSON string
import_from_photos_request_instance = ImportFromPhotosRequest.from_json(json)
# print the JSON string representation of the object
print(ImportFromPhotosRequest.to_json())

# convert the object into a dict
import_from_photos_request_dict = import_from_photos_request_instance.to_dict()
# create an instance of ImportFromPhotosRequest from a dict
import_from_photos_request_from_dict = ImportFromPhotosRequest.from_dict(import_from_photos_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


