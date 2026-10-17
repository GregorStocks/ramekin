# SyncMealPlansRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**cursor** | **int** | &#x60;cursor&#x60; from the previous sync. Absent means a full sync. | [optional] 
**ops** | [**List[MealPlanOp]**](MealPlanOp.md) | Writes to apply, in order, before reading changes. | [optional] 

## Example

```python
from ramekin_client.models.sync_meal_plans_request import SyncMealPlansRequest

# TODO update the JSON string below
json = "{}"
# create an instance of SyncMealPlansRequest from a JSON string
sync_meal_plans_request_instance = SyncMealPlansRequest.from_json(json)
# print the JSON string representation of the object
print(SyncMealPlansRequest.to_json())

# convert the object into a dict
sync_meal_plans_request_dict = sync_meal_plans_request_instance.to_dict()
# create an instance of SyncMealPlansRequest from a dict
sync_meal_plans_request_from_dict = SyncMealPlansRequest.from_dict(sync_meal_plans_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


