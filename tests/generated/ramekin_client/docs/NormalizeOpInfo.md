# NormalizeOpInfo

One normalize op, as the registry defines it.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**available** | **bool** | Whether the op can run on the recipe asked about; always true without a recipe. | 
**default_on_import** | **bool** | Whether imports and scrapes run the op automatically. | 
**group** | [**NormalizeOpGroupResponse**](NormalizeOpGroupResponse.md) |  | 
**id** | **str** |  | 
**implies** | **List[str]** | Ops that always run with this one. | 
**label** | **str** |  | 
**runs_after** | **List[str]** | Ops this one runs after when both run. | 
**unavailable_reason** | **str** |  | [optional] 
**uses_ai** | **bool** |  | 
**writes** | **List[str]** | Recipe fields the op can change. | 

## Example

```python
from ramekin_client.models.normalize_op_info import NormalizeOpInfo

# TODO update the JSON string below
json = "{}"
# create an instance of NormalizeOpInfo from a JSON string
normalize_op_info_instance = NormalizeOpInfo.from_json(json)
# print the JSON string representation of the object
print(NormalizeOpInfo.to_json())

# convert the object into a dict
normalize_op_info_dict = normalize_op_info_instance.to_dict()
# create an instance of NormalizeOpInfo from a dict
normalize_op_info_from_dict = NormalizeOpInfo.from_dict(normalize_op_info_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


