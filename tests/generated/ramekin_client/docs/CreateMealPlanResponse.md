# CreateMealPlanResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **UUID** |  | 

## Example

```python
from ramekin_client.models.create_meal_plan_response import CreateMealPlanResponse

# TODO update the JSON string below
json = "{}"
# create an instance of CreateMealPlanResponse from a JSON string
create_meal_plan_response_instance = CreateMealPlanResponse.from_json(json)
# print the JSON string representation of the object
print(CreateMealPlanResponse.to_json())

# convert the object into a dict
create_meal_plan_response_dict = create_meal_plan_response_instance.to_dict()
# create an instance of CreateMealPlanResponse from a dict
create_meal_plan_response_from_dict = CreateMealPlanResponse.from_dict(create_meal_plan_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


