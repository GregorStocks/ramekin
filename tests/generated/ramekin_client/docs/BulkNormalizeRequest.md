# BulkNormalizeRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**ops** | **List[str]** | Op ids to run on each recipe. | 
**recipe_ids** | **List[UUID]** |  | 

## Example

```python
from ramekin_client.models.bulk_normalize_request import BulkNormalizeRequest

# TODO update the JSON string below
json = "{}"
# create an instance of BulkNormalizeRequest from a JSON string
bulk_normalize_request_instance = BulkNormalizeRequest.from_json(json)
# print the JSON string representation of the object
print(BulkNormalizeRequest.to_json())

# convert the object into a dict
bulk_normalize_request_dict = bulk_normalize_request_instance.to_dict()
# create an instance of BulkNormalizeRequest from a dict
bulk_normalize_request_from_dict = BulkNormalizeRequest.from_dict(bulk_normalize_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


