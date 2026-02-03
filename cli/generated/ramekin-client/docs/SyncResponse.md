# SyncResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**created** | [**Vec<models::SyncCreatedItem>**](SyncCreatedItem.md) | Items that were created (maps client_id to server_id) | 
**deleted** | [**Vec<uuid::Uuid>**](uuid::Uuid.md) | IDs of items that were deleted | 
**server_changes** | [**Vec<models::SyncServerChange>**](SyncServerChange.md) | Server-side changes since last_sync_at | 
**sync_timestamp** | **String** | New sync timestamp to use for next sync | 
**updated** | [**Vec<models::SyncUpdatedItem>**](SyncUpdatedItem.md) | Items that were updated (with success status) | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


