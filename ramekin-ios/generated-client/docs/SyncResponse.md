# SyncResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**categoryOrder** | **[String]** | Canonical category display order for grouping items; every item&#39;s &#x60;category&#x60; is guaranteed to appear in this list. | 
**created** | [SyncCreatedItem] | Items that were created (maps client_id to server_id) | 
**cursor** | **Int64** | Snapshot watermark to pass as &#x60;cursor&#x60; on the next sync. Changes may be redelivered across syncs, but none can be skipped. | 
**deleted** | **[UUID]** | IDs of items that were deleted | 
**serverChanges** | [SyncServerChange] | Server-side changes since the request&#39;s cursor | 
**syncTimestamp** | **Date** | Server time when this sync started. Deprecated as a cursor: pass &#x60;cursor&#x60; instead. | 
**updated** | [SyncUpdatedItem] | Items that were updated (with success status) | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


