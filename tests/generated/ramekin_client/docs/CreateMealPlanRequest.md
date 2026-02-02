# CreateMealPlanRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**meal_date** | **date** |  | 
**meal_type** | [**MealType**](MealType.md) |  | 
**notes** | **str** |  | [optional] 
**recipe_id** | **UUID** |  | 

## Example

```python
from ramekin_client.models.create_meal_plan_request import CreateMealPlanRequest

# TODO update the JSON string below
json = "{}"
# create an instance of CreateMealPlanRequest from a JSON string
create_meal_plan_request_instance = CreateMealPlanRequest.from_json(json)
# print the JSON string representation of the object
print(CreateMealPlanRequest.to_json())

# convert the object into a dict
create_meal_plan_request_dict = create_meal_plan_request_instance.to_dict()
# create an instance of CreateMealPlanRequest from a dict
create_meal_plan_request_from_dict = CreateMealPlanRequest.from_dict(create_meal_plan_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


