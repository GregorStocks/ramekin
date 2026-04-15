# NormalizeTitleResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**cached** | **bool** |  | 
**changed** | **bool** |  | 
**normalized_title** | **str** |  | 
**original_title** | **str** |  | 

## Example

```python
from ramekin_client.models.normalize_title_response import NormalizeTitleResponse

# TODO update the JSON string below
json = "{}"
# create an instance of NormalizeTitleResponse from a JSON string
normalize_title_response_instance = NormalizeTitleResponse.from_json(json)
# print the JSON string representation of the object
print(NormalizeTitleResponse.to_json())

# convert the object into a dict
normalize_title_response_dict = normalize_title_response_instance.to_dict()
# create an instance of NormalizeTitleResponse from a dict
normalize_title_response_from_dict = NormalizeTitleResponse.from_dict(normalize_title_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


