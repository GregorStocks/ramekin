# UpdateMealPlanRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**meal_date** | **date** |  | [optional] 
**meal_type** | [**MealType**](MealType.md) |  | [optional] 
**notes** | **str** | Set to empty string to clear notes, or provide new value | [optional] 

## Example

```python
from ramekin_client.models.update_meal_plan_request import UpdateMealPlanRequest

# TODO update the JSON string below
json = "{}"
# create an instance of UpdateMealPlanRequest from a JSON string
update_meal_plan_request_instance = UpdateMealPlanRequest.from_json(json)
# print the JSON string representation of the object
print(UpdateMealPlanRequest.to_json())

# convert the object into a dict
update_meal_plan_request_dict = update_meal_plan_request_instance.to_dict()
# create an instance of UpdateMealPlanRequest from a dict
update_meal_plan_request_from_dict = UpdateMealPlanRequest.from_dict(update_meal_plan_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


