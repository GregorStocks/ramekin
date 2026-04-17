# GeneratePhotoResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**photo_id** | **UUID** |  | 
**version_id** | **UUID** |  | 

## Example

```python
from ramekin_client.models.generate_photo_response import GeneratePhotoResponse

# TODO update the JSON string below
json = "{}"
# create an instance of GeneratePhotoResponse from a JSON string
generate_photo_response_instance = GeneratePhotoResponse.from_json(json)
# print the JSON string representation of the object
print(GeneratePhotoResponse.to_json())

# convert the object into a dict
generate_photo_response_dict = generate_photo_response_instance.to_dict()
# create an instance of GeneratePhotoResponse from a dict
generate_photo_response_from_dict = GeneratePhotoResponse.from_dict(generate_photo_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


