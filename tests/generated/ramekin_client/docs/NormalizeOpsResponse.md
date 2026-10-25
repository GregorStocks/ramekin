# NormalizeOpsResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**ops** | [**List[NormalizeOpInfo]**](NormalizeOpInfo.md) | Every op, in run order. | 

## Example

```python
from ramekin_client.models.normalize_ops_response import NormalizeOpsResponse

# TODO update the JSON string below
json = "{}"
# create an instance of NormalizeOpsResponse from a JSON string
normalize_ops_response_instance = NormalizeOpsResponse.from_json(json)
# print the JSON string representation of the object
print(NormalizeOpsResponse.to_json())

# convert the object into a dict
normalize_ops_response_dict = normalize_ops_response_instance.to_dict()
# create an instance of NormalizeOpsResponse from a dict
normalize_ops_response_from_dict = NormalizeOpsResponse.from_dict(normalize_ops_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


