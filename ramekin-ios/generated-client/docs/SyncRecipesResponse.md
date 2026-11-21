# SyncRecipesResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**afterId** | **UUID** | Set while &#x60;has_more&#x60;: the last recipe of this page, which shares its change stamp with recipes still to come. Pass it back with &#x60;cursor&#x60;. | [optional] 
**changes** | [SyncRecipe] | The next &#x60;limit&#x60; active recipes changed since the request&#39;s position, oldest change first. A recipe changed again later comes again, later in the stream, so paging can&#39;t skip a change that lands mid-sync. | 
**cursor** | **Int64** | Where the next request starts: pass it back as &#x60;cursor&#x60;, together with &#x60;after_id&#x60;. Persist the pair once this page is applied; an interrupted sync resumes from it. Changes may be redelivered, but none can be skipped. | 
**deleted** | **[UUID]** | Recipe IDs deleted since the request&#39;s position. A full resync sends none. | 
**fullResync** | **Bool** | True when the request&#39;s cursor couldn&#39;t be answered with a delta: it was absent, predates a restored backup, or came from another database. The stream then starts over from every active recipe, so the client must drop everything it has cached before applying this page (recipes the server lost have no tombstone). Later pages of the same sync are ordinary deltas. | 
**hasMore** | **Bool** | True when more changes follow this page. Request them right away. | 
**normalizationContractVersion** | **Int** | Version of the shared search-normalization contract (shared-test-vectors/search-normalization.json) the server was built with. A client mirroring server search locally must fail the sync when it does not support this version: matching with a stale contract would silently drop or add results relative to server search. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


