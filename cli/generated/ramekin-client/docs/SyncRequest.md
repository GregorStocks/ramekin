# SyncRequest

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**creates** | Option<[**Vec<models::SyncCreateItem>**](SyncCreateItem.md)> | Items created offline | [optional]
**deletes** | Option<[**Vec<uuid::Uuid>**](uuid::Uuid.md)> | IDs of items deleted offline | [optional]
**last_sync_at** | Option<**String**> | Last sync timestamp - server will return changes since this time | [optional]
**updates** | Option<[**Vec<models::SyncUpdateItem>**](SyncUpdateItem.md)> | Items updated offline | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


