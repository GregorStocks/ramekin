# UploadPhotoResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **UUID** |  | 

## Example

```python
from ramekin_client.models.upload_photo_response import UploadPhotoResponse

# TODO update the JSON string below
json = "{}"
# create an instance of UploadPhotoResponse from a JSON string
upload_photo_response_instance = UploadPhotoResponse.from_json(json)
# print the JSON string representation of the object
print(UploadPhotoResponse.to_json())

# convert the object into a dict
upload_photo_response_dict = upload_photo_response_instance.to_dict()
# create an instance of UploadPhotoResponse from a dict
upload_photo_response_from_dict = UploadPhotoResponse.from_dict(upload_photo_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


