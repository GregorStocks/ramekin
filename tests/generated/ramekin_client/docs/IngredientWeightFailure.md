# IngredientWeightFailure


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**attempts** | **int** |  | 
**error** | **str** |  | 
**food** | **str** | The catalog food, as the catalog names it. | 
**unit** | **str** |  | 

## Example

```python
from ramekin_client.models.ingredient_weight_failure import IngredientWeightFailure

# TODO update the JSON string below
json = "{}"
# create an instance of IngredientWeightFailure from a JSON string
ingredient_weight_failure_instance = IngredientWeightFailure.from_json(json)
# print the JSON string representation of the object
print(IngredientWeightFailure.to_json())

# convert the object into a dict
ingredient_weight_failure_dict = ingredient_weight_failure_instance.to_dict()
# create an instance of IngredientWeightFailure from a dict
ingredient_weight_failure_from_dict = IngredientWeightFailure.from_dict(ingredient_weight_failure_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


