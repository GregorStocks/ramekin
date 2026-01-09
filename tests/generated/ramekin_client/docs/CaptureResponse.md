# CaptureResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**recipe_id** | **UUID** | The created recipe ID | 
**title** | **str** | The extracted recipe title | 

## Example

```python
from ramekin_client.models.capture_response import CaptureResponse

# TODO update the JSON string below
json = "{}"
# create an instance of CaptureResponse from a JSON string
capture_response_instance = CaptureResponse.from_json(json)
# print the JSON string representation of the object
print(CaptureResponse.to_json())

# convert the object into a dict
capture_response_dict = capture_response_instance.to_dict()
# create an instance of CaptureResponse from a dict
capture_response_from_dict = CaptureResponse.from_dict(capture_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


