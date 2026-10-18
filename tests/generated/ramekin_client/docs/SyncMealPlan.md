# SyncMealPlan


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**change_seq** | **int** | Sync stamp of the plan&#39;s own last change. A cached copy with a higher one is newer than this row. | 
**id** | **UUID** |  | 
**meal_date** | **date** |  | 
**meal_type** | [**MealType**](MealType.md) |  | 
**notes** | **str** |  | [optional] 
**recipe_id** | **UUID** |  | 
**recipe_title** | **str** | The recipe&#39;s current title and first photo. A new recipe version resends the plans that use it. | 
**thumbnail_photo_id** | **UUID** |  | [optional] 

## Example

```python
from ramekin_client.models.sync_meal_plan import SyncMealPlan

# TODO update the JSON string below
json = "{}"
# create an instance of SyncMealPlan from a JSON string
sync_meal_plan_instance = SyncMealPlan.from_json(json)
# print the JSON string representation of the object
print(SyncMealPlan.to_json())

# convert the object into a dict
sync_meal_plan_dict = sync_meal_plan_instance.to_dict()
# create an instance of SyncMealPlan from a dict
sync_meal_plan_from_dict = SyncMealPlan.from_dict(sync_meal_plan_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


