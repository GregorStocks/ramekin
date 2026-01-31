# Ingredient

Ingredient structure for JSONB storage

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**item** | **str** | The ingredient name (e.g., \&quot;butter\&quot;, \&quot;all-purpose flour\&quot;) | 
**measurements** | [**List[Measurement]**](Measurement.md) | Measurements - first is primary, rest are alternatives (e.g., \&quot;1 stick\&quot; then \&quot;113g\&quot;) | 
**note** | **str** | Preparation notes (e.g., \&quot;chopped\&quot;, \&quot;softened\&quot;, \&quot;optional\&quot;) | [optional] 
**raw** | **str** | Original unparsed text for debugging | [optional] 
**section** | **str** | Section name for grouping (e.g., \&quot;For the sauce\&quot;, \&quot;For the dough\&quot;) | [optional] 

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


