# IngredientNamesQueuedResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**queued** | **int** | Names queued for resolution by this call. | 

## Example

```python
from ramekin_client.models.ingredient_names_queued_response import IngredientNamesQueuedResponse

# TODO update the JSON string below
json = "{}"
# create an instance of IngredientNamesQueuedResponse from a JSON string
ingredient_names_queued_response_instance = IngredientNamesQueuedResponse.from_json(json)
# print the JSON string representation of the object
print(IngredientNamesQueuedResponse.to_json())

# convert the object into a dict
ingredient_names_queued_response_dict = ingredient_names_queued_response_instance.to_dict()
# create an instance of IngredientNamesQueuedResponse from a dict
ingredient_names_queued_response_from_dict = IngredientNamesQueuedResponse.from_dict(ingredient_names_queued_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


