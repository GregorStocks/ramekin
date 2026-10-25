# AcceptNormalizeResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**version_id** | **UUID** | The new current version. | 

## Example

```python
from ramekin_client.models.accept_normalize_response import AcceptNormalizeResponse

# TODO update the JSON string below
json = "{}"
# create an instance of AcceptNormalizeResponse from a JSON string
accept_normalize_response_instance = AcceptNormalizeResponse.from_json(json)
# print the JSON string representation of the object
print(AcceptNormalizeResponse.to_json())

# convert the object into a dict
accept_normalize_response_dict = accept_normalize_response_instance.to_dict()
# create an instance of AcceptNormalizeResponse from a dict
accept_normalize_response_from_dict = AcceptNormalizeResponse.from_dict(accept_normalize_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


