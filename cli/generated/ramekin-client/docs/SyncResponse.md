# SyncResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**category_order** | **Vec<String>** | Canonical category display order for grouping items; every item's `category` is guaranteed to appear in this list. | 
**created** | [**Vec<models::SyncCreatedItem>**](SyncCreatedItem.md) | Items that were created (maps client_id to server_id) | 
**cursor** | **i64** | Snapshot watermark to pass as `cursor` on the next sync. Changes may be redelivered across syncs, but none can be skipped. | 
**deleted** | [**Vec<uuid::Uuid>**](uuid::Uuid.md) | IDs of items that were deleted | 
**server_changes** | [**Vec<models::SyncServerChange>**](SyncServerChange.md) | Server-side changes since the request's cursor | 
**sync_timestamp** | **String** | Server time when this sync started. Deprecated as a cursor: pass `cursor` instead. | 
**updated** | [**Vec<models::SyncUpdatedItem>**](SyncUpdatedItem.md) | Items that were updated (with success status) | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


