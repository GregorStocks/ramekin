# MealPlanFields

A meal plan's writable fields. On `create`, `recipe_id`, `meal_date` and `meal_type` are required. On `update`, only the fields present change, and `recipe_id` can't. `notes` is cleared with \"\".

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**meal_date** | **date** |  | [optional] 
**meal_type** | [**MealType**](MealType.md) |  | [optional] 
**notes** | **str** |  | [optional] 
**recipe_id** | **UUID** |  | [optional] 

## Example

```python
from ramekin_client.models.meal_plan_fields import MealPlanFields

# TODO update the JSON string below
json = "{}"
# create an instance of MealPlanFields from a JSON string
meal_plan_fields_instance = MealPlanFields.from_json(json)
# print the JSON string representation of the object
print(MealPlanFields.to_json())

# convert the object into a dict
meal_plan_fields_dict = meal_plan_fields_instance.to_dict()
# create an instance of MealPlanFields from a dict
meal_plan_fields_from_dict = MealPlanFields.from_dict(meal_plan_fields_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


