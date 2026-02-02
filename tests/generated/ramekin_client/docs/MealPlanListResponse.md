# MealPlanListResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**meal_plans** | [**List[MealPlanItem]**](MealPlanItem.md) |  | 

## Example

```python
from ramekin_client.models.meal_plan_list_response import MealPlanListResponse

# TODO update the JSON string below
json = "{}"
# create an instance of MealPlanListResponse from a JSON string
meal_plan_list_response_instance = MealPlanListResponse.from_json(json)
# print the JSON string representation of the object
print(MealPlanListResponse.to_json())

# convert the object into a dict
meal_plan_list_response_dict = meal_plan_list_response_instance.to_dict()
# create an instance of MealPlanListResponse from a dict
meal_plan_list_response_from_dict = MealPlanListResponse.from_dict(meal_plan_list_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


