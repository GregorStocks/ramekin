# SyncRecipesResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**cursor** | **int** | This page&#39;s snapshot watermark. Once a sweep completes, persist the *first* page&#39;s cursor and pass it to the next sync: changes committed mid-sweep can land in id ranges the sweep already passed, and only the first watermark is low enough to redeliver all of them. Changes may be redelivered across syncs, but none can be skipped. | 
**deleted** | **List[UUID]** | Recipe IDs deleted at or after &#x60;cursor&#x60;. Only sent on a sweep&#39;s first page; later pages return an empty list. | 
**has_more** | **bool** | True when the sweep has more pages. Request the next one with the same &#x60;cursor&#x60; and &#x60;after_id&#x60; set to this page&#39;s last recipe ID. | 
**recipes** | [**List[SyncRecipe]**](SyncRecipe.md) | The next &#x60;limit&#x60; active recipes (by ascending recipe ID, starting past &#x60;after_id&#x60;) changed at or after &#x60;cursor&#x60;. All active recipes match when &#x60;cursor&#x60; is absent. | 

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


