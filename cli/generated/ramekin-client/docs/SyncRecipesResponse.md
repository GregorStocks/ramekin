# SyncRecipesResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**after_id** | Option<[**uuid::Uuid**](uuid::Uuid.md)> | Set while `has_more`: the last recipe of this page, which shares its change stamp with recipes still to come. Pass it back with `cursor`. | [optional]
**changes** | [**Vec<models::SyncRecipe>**](SyncRecipe.md) | The next `limit` active recipes changed since the request's position, oldest change first. A recipe changed again later comes again, later in the stream, so paging can't skip a change that lands mid-sync. | 
**cursor** | **i64** | Where the next request starts: pass it back as `cursor`, together with `after_id`. Persist the pair once this page is applied; an interrupted sync resumes from it. Changes may be redelivered, but none can be skipped. | 
**deleted** | [**Vec<uuid::Uuid>**](uuid::Uuid.md) | Recipe IDs deleted since the request's position. A full resync sends none. | 
**full_resync** | **bool** | True when the request's cursor couldn't be answered with a delta: it was absent, predates a restored backup, or came from another database. The stream then starts over from every active recipe, so the client must drop everything it has cached before applying this page (recipes the server lost have no tombstone). Later pages of the same sync are ordinary deltas. | 
**has_more** | **bool** | True when more changes follow this page. Request them right away. | 
**normalization_contract_version** | **i32** | Version of the shared search-normalization contract (shared-test-vectors/search-normalization.json) the server was built with. A client mirroring server search locally must fail the sync when it does not support this version: matching with a stale contract would silently drop or add results relative to server search. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


