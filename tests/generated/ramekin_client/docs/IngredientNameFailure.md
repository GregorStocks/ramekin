# IngredientNameFailure


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**attempts** | **int** |  | 
**error** | **str** |  | 
**name** | **str** |  | 

## Example

```python
from ramekin_client.models.ingredient_name_failure import IngredientNameFailure

# TODO update the JSON string below
json = "{}"
# create an instance of IngredientNameFailure from a JSON string
ingredient_name_failure_instance = IngredientNameFailure.from_json(json)
# print the JSON string representation of the object
print(IngredientNameFailure.to_json())

# convert the object into a dict
ingredient_name_failure_dict = ingredient_name_failure_instance.to_dict()
# create an instance of IngredientNameFailure from a dict
ingredient_name_failure_from_dict = IngredientNameFailure.from_dict(ingredient_name_failure_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


