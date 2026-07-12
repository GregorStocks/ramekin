# SyncRecipesResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**cursor** | **int** | Opaque cursor to pass to the next sync. Changes may be redelivered across syncs, but none can be skipped. | 
**deleted** | **List[UUID]** | Recipe IDs deleted at or after &#x60;cursor&#x60;. | 
**recipes** | [**List[SyncRecipe]**](SyncRecipe.md) | Active recipes changed at or after &#x60;cursor&#x60;. All active recipes are returned when &#x60;cursor&#x60; is absent. | 

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


