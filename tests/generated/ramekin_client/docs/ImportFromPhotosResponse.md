# ImportFromPhotosResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**job_id** | **UUID** | The created job ID | 
**status** | **str** | Current job status | 

## Example

```python
from ramekin_client.models.import_from_photos_response import ImportFromPhotosResponse

# TODO update the JSON string below
json = "{}"
# create an instance of ImportFromPhotosResponse from a JSON string
import_from_photos_response_instance = ImportFromPhotosResponse.from_json(json)
# print the JSON string representation of the object
print(ImportFromPhotosResponse.to_json())

# convert the object into a dict
import_from_photos_response_dict = import_from_photos_response_instance.to_dict()
# create an instance of ImportFromPhotosResponse from a dict
import_from_photos_response_from_dict = ImportFromPhotosResponse.from_dict(import_from_photos_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


