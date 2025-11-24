# HelloResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**message** | **str** |  | 

## Example

```python
from ramekin_client.models.hello_response import HelloResponse

# TODO update the JSON string below
json = "{}"
# create an instance of HelloResponse from a JSON string
hello_response_instance = HelloResponse.from_json(json)
# print the JSON string representation of the object
print(HelloResponse.to_json())

# convert the object into a dict
hello_response_dict = hello_response_instance.to_dict()
# create an instance of HelloResponse from a dict
hello_response_from_dict = HelloResponse.from_dict(hello_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


