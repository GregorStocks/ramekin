# SyncRecipesResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**changes** | [**Vec<models::SyncRecipe>**](SyncRecipe.md) | The next `limit` active recipes (by ascending recipe ID, starting past `after_id`) changed at or after `cursor`. All active recipes match when `cursor` is absent. | 
**cursor** | **i64** | This page's snapshot watermark. Once a sweep completes, persist the *first* page's cursor and pass it to the next sync: changes committed mid-sweep can land in id ranges the sweep already passed, and only the first watermark is low enough to redeliver all of them. Changes may be redelivered across syncs, but none can be skipped. | 
**deleted** | [**Vec<uuid::Uuid>**](uuid::Uuid.md) | Recipe IDs deleted at or after `cursor`. Only sent on a delta sweep's first page; later pages and full resyncs return an empty list. | 
**full_resync** | **bool** | True when this page lists every active recipe in its ID range rather than changes since `cursor`: `cursor` was absent, predates a restored backup, or came from another database. The client must then drop cached recipes the sweep doesn't return (a full resync sends no tombstones) and request the sweep's remaining pages without `cursor`, since a later page could otherwise judge the same cursor differently. Pages are ordered by ID, so each settles the ID range from `after_id` (exclusive) through its last recipe, or to the end of the ID space on the last page. | 
**has_more** | **bool** | True when the sweep has more pages. Request the next one with the same `cursor` and `after_id` set to this page's last recipe ID. | 
**normalization_contract_version** | **i32** | Version of the shared search-normalization contract (shared-test-vectors/search-normalization.json) the server was built with. A client mirroring server search locally must fail the sync when it does not support this version: matching with a stale contract would silently drop or add results relative to server search. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


