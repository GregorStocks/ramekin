# MealPlanItem


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **UUID** |  | 
**meal_date** | **date** |  | 
**meal_type** | [**MealType**](MealType.md) |  | 
**notes** | **str** |  | [optional] 
**recipe_id** | **UUID** |  | 
**recipe_title** | **str** |  | 
**thumbnail_photo_id** | **UUID** |  | [optional] 

## Example

```python
from ramekin_client.models.meal_plan_item import MealPlanItem

# TODO update the JSON string below
json = "{}"
# create an instance of MealPlanItem from a JSON string
meal_plan_item_instance = MealPlanItem.from_json(json)
# print the JSON string representation of the object
print(MealPlanItem.to_json())

# convert the object into a dict
meal_plan_item_dict = meal_plan_item_instance.to_dict()
# create an instance of MealPlanItem from a dict
meal_plan_item_from_dict = MealPlanItem.from_dict(meal_plan_item_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


