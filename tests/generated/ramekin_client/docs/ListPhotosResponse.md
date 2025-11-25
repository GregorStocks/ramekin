# ListPhotosResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**photos** | [**List[PhotoSummary]**](PhotoSummary.md) |  | 

## Example

```python
from ramekin_client.models.list_photos_response import ListPhotosResponse

# TODO update the JSON string below
json = "{}"
# create an instance of ListPhotosResponse from a JSON string
list_photos_response_instance = ListPhotosResponse.from_json(json)
# print the JSON string representation of the object
print(ListPhotosResponse.to_json())

# convert the object into a dict
list_photos_response_dict = list_photos_response_instance.to_dict()
# create an instance of ListPhotosResponse from a dict
list_photos_response_from_dict = ListPhotosResponse.from_dict(list_photos_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


