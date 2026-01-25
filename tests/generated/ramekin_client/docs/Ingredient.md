# Ingredient

Structured ingredient with parsed amount, unit, item, and note.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**amount** | **str** |  | [optional] 
**item** | **str** |  | 
**note** | **str** |  | [optional] 
**unit** | **str** |  | [optional] 

## Example

```python
from ramekin_client.models.ingredient import Ingredient

# TODO update the JSON string below
json = "{}"
# create an instance of Ingredient from a JSON string
ingredient_instance = Ingredient.from_json(json)
# print the JSON string representation of the object
print(Ingredient.to_json())

# convert the object into a dict
ingredient_dict = ingredient_instance.to_dict()
# create an instance of Ingredient from a dict
ingredient_from_dict = Ingredient.from_dict(ingredient_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


