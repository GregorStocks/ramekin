# IngredientNamesStatusResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**estimated** | **int** | A real food no catalog entry matches, counted with the model&#39;s own calories (\&quot;estimated calories\&quot;). | 
**failed** | **int** | The last attempt failed; retry to try again. | 
**failures** | [**List[IngredientNameFailure]**](IngredientNameFailure.md) | The most recent failures, failed re-asks included. | 
**not_food** | **int** | Resolved as not an ingredient (a heading, a serving note). | 
**pending** | **int** | Waiting for the background resolver. | 
**reask_failed** | **int** | Re-asking with the current model failed; the earlier model&#39;s answer is still used (and counted above). Retry to try again. | 
**reasking** | **int** | Being asked again by the current model; the earlier model&#39;s answer is used meanwhile (and counted above). | 
**recognized** | **int** | Resolved to a catalog food or product (for an ambiguous name, the one a recipe most likely means). | 
**unknown** | **int** | Resolved, but the model couldn&#39;t tell; still unknown in estimates. | 
**weights** | [**IngredientWeightsStatus**](IngredientWeightsStatus.md) |  | 

## Example

```python
from ramekin_client.models.ingredient_names_status_response import IngredientNamesStatusResponse

# TODO update the JSON string below
json = "{}"
# create an instance of IngredientNamesStatusResponse from a JSON string
ingredient_names_status_response_instance = IngredientNamesStatusResponse.from_json(json)
# print the JSON string representation of the object
print(IngredientNamesStatusResponse.to_json())

# convert the object into a dict
ingredient_names_status_response_dict = ingredient_names_status_response_instance.to_dict()
# create an instance of IngredientNamesStatusResponse from a dict
ingredient_names_status_response_from_dict = IngredientNamesStatusResponse.from_dict(ingredient_names_status_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


