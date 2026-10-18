# MealPlanOp


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**fields** | [**MealPlanFields**](MealPlanFields.md) |  | [optional] 
**id** | **UUID** | The plan&#39;s id. A create chooses it. | 
**op** | [**OpKind**](OpKind.md) |  | 

## Example

```python
from ramekin_client.models.meal_plan_op import MealPlanOp

# TODO update the JSON string below
json = "{}"
# create an instance of MealPlanOp from a JSON string
meal_plan_op_instance = MealPlanOp.from_json(json)
# print the JSON string representation of the object
print(MealPlanOp.to_json())

# convert the object into a dict
meal_plan_op_dict = meal_plan_op_instance.to_dict()
# create an instance of MealPlanOp from a dict
meal_plan_op_from_dict = MealPlanOp.from_dict(meal_plan_op_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


