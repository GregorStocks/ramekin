# NormalizeRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**ops** | **List[str]** | Op ids to run. Implied ops are added, and the registry orders them. | 

## Example

```python
from ramekin_client.models.normalize_request import NormalizeRequest

# TODO update the JSON string below
json = "{}"
# create an instance of NormalizeRequest from a JSON string
normalize_request_instance = NormalizeRequest.from_json(json)
# print the JSON string representation of the object
print(NormalizeRequest.to_json())

# convert the object into a dict
normalize_request_dict = normalize_request_instance.to_dict()
# create an instance of NormalizeRequest from a dict
normalize_request_from_dict = NormalizeRequest.from_dict(normalize_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


