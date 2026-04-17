# GenerateDescriptionResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**cached** | **bool** |  | 
**changed** | **bool** |  | 
**generated_description** | **str** |  | 
**original_description** | **str** |  | [optional] 

## Example

```python
from ramekin_client.models.generate_description_response import GenerateDescriptionResponse

# TODO update the JSON string below
json = "{}"
# create an instance of GenerateDescriptionResponse from a JSON string
generate_description_response_instance = GenerateDescriptionResponse.from_json(json)
# print the JSON string representation of the object
print(GenerateDescriptionResponse.to_json())

# convert the object into a dict
generate_description_response_dict = generate_description_response_instance.to_dict()
# create an instance of GenerateDescriptionResponse from a dict
generate_description_response_from_dict = GenerateDescriptionResponse.from_dict(generate_description_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


