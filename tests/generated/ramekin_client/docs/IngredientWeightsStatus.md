# IngredientWeightsStatus

Weights the catalog lacks for foods in the user's recipes (a density, or a counted unit such as \"bunch\"), estimated in the background.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**estimated** | **int** | Estimated; lines using one say \&quot;estimated weight\&quot;. | 
**failed** | **int** | The last attempt failed; retry to try again. | 
**failures** | [**List[IngredientWeightFailure]**](IngredientWeightFailure.md) | The most recent failures. | 
**no_typical_weight** | **int** | The model said there&#39;s no typical weight; still unknown in estimates. | 
**pending** | **int** | Waiting for the background estimator (queued when an estimate is shown). | 

## Example

```python
from ramekin_client.models.ingredient_weights_status import IngredientWeightsStatus

# TODO update the JSON string below
json = "{}"
# create an instance of IngredientWeightsStatus from a JSON string
ingredient_weights_status_instance = IngredientWeightsStatus.from_json(json)
# print the JSON string representation of the object
print(IngredientWeightsStatus.to_json())

# convert the object into a dict
ingredient_weights_status_dict = ingredient_weights_status_instance.to_dict()
# create an instance of IngredientWeightsStatus from a dict
ingredient_weights_status_from_dict = IngredientWeightsStatus.from_dict(ingredient_weights_status_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


