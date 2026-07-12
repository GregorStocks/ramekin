# SyncRecipesResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**cursor** | **Int64** | This page&#39;s snapshot watermark. Once a sweep completes, persist the *first* page&#39;s cursor and pass it to the next sync: changes committed mid-sweep can land in id ranges the sweep already passed, and only the first watermark is low enough to redeliver all of them. Changes may be redelivered across syncs, but none can be skipped. | 
**deleted** | **[UUID]** | Recipe IDs deleted at or after &#x60;cursor&#x60;. Only sent on a sweep&#39;s first page; later pages return an empty list. | 
**hasMore** | **Bool** | True when the sweep has more pages. Request the next one with the same &#x60;cursor&#x60; and &#x60;after_id&#x60; set to this page&#39;s last recipe ID. | 
**normalizationContractVersion** | **Int** | Version of the shared search-normalization contract (shared-test-vectors/search-normalization.json) the server was built with. A client mirroring server search locally must fail the sync when it does not support this version: matching with a stale contract would silently drop or add results relative to server search. | 
**recipes** | [SyncRecipe] | The next &#x60;limit&#x60; active recipes (by ascending recipe ID, starting past &#x60;after_id&#x60;) changed at or after &#x60;cursor&#x60;. All active recipes match when &#x60;cursor&#x60; is absent. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


