# SyncRecipesResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**deleted** | **List[UUID]** | Recipe IDs deleted since last_sync_at. | 
**recipes** | [**List[RecipeSummary]**](RecipeSummary.md) | Active recipes created or updated since last_sync_at. All active recipes are returned when last_sync_at is absent. | 
**sync_timestamp** | **datetime** | New sync timestamp to use for the next sync. | 

## Example

```python
from ramekin_client.models.sync_recipes_response import SyncRecipesResponse

# TODO update the JSON string below
json = "{}"
# create an instance of SyncRecipesResponse from a JSON string
sync_recipes_response_instance = SyncRecipesResponse.from_json(json)
# print the JSON string representation of the object
print(SyncRecipesResponse.to_json())

# convert the object into a dict
sync_recipes_response_dict = sync_recipes_response_instance.to_dict()
# create an instance of SyncRecipesResponse from a dict
sync_recipes_response_from_dict = SyncRecipesResponse.from_dict(sync_recipes_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


