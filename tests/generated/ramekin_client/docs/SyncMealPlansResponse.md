# SyncMealPlansResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**changes** | [**List[SyncMealPlan]**](SyncMealPlan.md) | Live plans (of live recipes) changed at or after the request&#39;s &#x60;cursor&#x60;, this request&#39;s own writes and every refused op&#39;s live plan included; every live plan on a full resync. | 
**cursor** | **int** | Pass as &#x60;cursor&#x60; on the next sync. | 
**deleted** | **List[UUID]** | Plan IDs deleted, or whose recipe was deleted, at or after &#x60;cursor&#x60;, and every refused op&#39;s plan that isn&#39;t live. Only the latter on a full resync. | 
**full_resync** | **bool** | True when &#x60;changes&#x60; holds every live plan: drop cached plans it omits. | 
**results** | [**List[OpResult]**](OpResult.md) | One per request op, in order. | 

## Example

```python
from ramekin_client.models.sync_meal_plans_response import SyncMealPlansResponse

# TODO update the JSON string below
json = "{}"
# create an instance of SyncMealPlansResponse from a JSON string
sync_meal_plans_response_instance = SyncMealPlansResponse.from_json(json)
# print the JSON string representation of the object
print(SyncMealPlansResponse.to_json())

# convert the object into a dict
sync_meal_plans_response_dict = sync_meal_plans_response_instance.to_dict()
# create an instance of SyncMealPlansResponse from a dict
sync_meal_plans_response_from_dict = SyncMealPlansResponse.from_dict(sync_meal_plans_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


