# BulkNormalizeResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**results** | [**List[BulkNormalizeResult]**](BulkNormalizeResult.md) |  | 

## Example

```python
from ramekin_client.models.bulk_normalize_response import BulkNormalizeResponse

# TODO update the JSON string below
json = "{}"
# create an instance of BulkNormalizeResponse from a JSON string
bulk_normalize_response_instance = BulkNormalizeResponse.from_json(json)
# print the JSON string representation of the object
print(BulkNormalizeResponse.to_json())

# convert the object into a dict
bulk_normalize_response_dict = bulk_normalize_response_instance.to_dict()
# create an instance of BulkNormalizeResponse from a dict
bulk_normalize_response_from_dict = BulkNormalizeResponse.from_dict(bulk_normalize_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


