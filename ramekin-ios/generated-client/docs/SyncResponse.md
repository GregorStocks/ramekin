# SyncResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**created** | [SyncCreatedItem] | Items that were created (maps client_id to server_id) | 
**deleted** | **[UUID]** | IDs of items that were deleted | 
**serverChanges** | [SyncServerChange] | Server-side changes since last_sync_at | 
**syncTimestamp** | **Date** | New sync timestamp to use for next sync | 
**updated** | [SyncUpdatedItem] | Items that were updated (with success status) | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


