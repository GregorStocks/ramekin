# UnknownCalorieIngredient


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**index** | **int** |  | 
**item** | **str** |  | 
**reason** | **str** |  | 

## Example

```python
from ramekin_client.models.unknown_calorie_ingredient import UnknownCalorieIngredient

# TODO update the JSON string below
json = "{}"
# create an instance of UnknownCalorieIngredient from a JSON string
unknown_calorie_ingredient_instance = UnknownCalorieIngredient.from_json(json)
# print the JSON string representation of the object
print(UnknownCalorieIngredient.to_json())

# convert the object into a dict
unknown_calorie_ingredient_dict = unknown_calorie_ingredient_instance.to_dict()
# create an instance of UnknownCalorieIngredient from a dict
unknown_calorie_ingredient_from_dict = UnknownCalorieIngredient.from_dict(unknown_calorie_ingredient_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


