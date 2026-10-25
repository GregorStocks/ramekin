# BulkNormalizeResult


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**error** | **str** | Why no job started for this recipe. | [optional] 
**job_id** | **UUID** |  | [optional] 
**recipe_id** | **UUID** |  | 

## Example

```python
from ramekin_client.models.bulk_normalize_result import BulkNormalizeResult

# TODO update the JSON string below
json = "{}"
# create an instance of BulkNormalizeResult from a JSON string
bulk_normalize_result_instance = BulkNormalizeResult.from_json(json)
# print the JSON string representation of the object
print(BulkNormalizeResult.to_json())

# convert the object into a dict
bulk_normalize_result_dict = bulk_normalize_result_instance.to_dict()
# create an instance of BulkNormalizeResult from a dict
bulk_normalize_result_from_dict = BulkNormalizeResult.from_dict(bulk_normalize_result_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


