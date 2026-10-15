# SyncResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**categoryOrder** | **[String]** | Canonical category display order for grouping items; every item&#39;s &#x60;category&#x60; is guaranteed to appear in this list. | 
**created** | [SyncCreatedItem] | Items that were created (maps client_id to server_id) | 
**cursor** | **Int64** | Snapshot watermark to pass as &#x60;cursor&#x60; on the next sync. Changes may be redelivered across syncs, but none can be skipped. | 
**deleted** | **[UUID]** | IDs of items that were deleted | 
**fullResync** | **Bool** | True when &#x60;server_changes&#x60; holds every live item rather than changes since the request&#39;s cursor: no cursor was sent, or it predates a restored backup or import, or came from another database. The client must then drop synced cached items missing from &#x60;server_changes&#x60; — an item rolled back by a restore is in neither &#x60;server_changes&#x60; nor &#x60;deleted&#x60;. | 
**serverChanges** | [SyncServerChange] | Server-side changes since the request&#39;s cursor | 
**syncTimestamp** | **Date** | Server time when this sync started. Deprecated as a cursor: pass &#x60;cursor&#x60; instead. | 
**updated** | [SyncUpdatedItem] | Items that were updated (with success status) | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


