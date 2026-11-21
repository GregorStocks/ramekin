# SyncRecipesResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**after_id** | **UUID** | Set while &#x60;has_more&#x60;: the last recipe of this page, which shares its change stamp with recipes still to come. Pass it back with &#x60;cursor&#x60;. | [optional] 
**changes** | [**List[SyncRecipe]**](SyncRecipe.md) | The next &#x60;limit&#x60; active recipes changed since the request&#39;s position, oldest change first. A recipe changed again later comes again, later in the stream, so paging can&#39;t skip a change that lands mid-sync. | 
**cursor** | **int** | Where the next request starts: pass it back as &#x60;cursor&#x60;, together with &#x60;after_id&#x60;. Persist the pair once this page is applied; an interrupted sync resumes from it. Changes may be redelivered, but none can be skipped. | 
**deleted** | **List[UUID]** | Recipe IDs deleted since the request&#39;s position. A full resync sends none. | 
**full_resync** | **bool** | True when the request&#39;s cursor couldn&#39;t be answered with a delta: it was absent, predates a restored backup, or came from another database. The stream then starts over from every active recipe, so the client must drop everything it has cached before applying this page (recipes the server lost have no tombstone). Later pages of the same sync are ordinary deltas. | 
**has_more** | **bool** | True when more changes follow this page. Request them right away. | 
**normalization_contract_version** | **int** | Version of the shared search-normalization contract (shared-test-vectors/search-normalization.json) the server was built with. A client mirroring server search locally must fail the sync when it does not support this version: matching with a stale contract would silently drop or add results relative to server search. | 

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


