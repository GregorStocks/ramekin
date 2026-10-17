# SyncRequest

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**creates** | Option<[**Vec<models::SyncCreateItem>**](SyncCreateItem.md)> | Items created offline | [optional]
**cursor** | Option<**i64**> | `cursor` from the previous sync's response. Server returns changes at or after it; without one, it returns every item. | [optional]
**deletes** | Option<[**Vec<uuid::Uuid>**](uuid::Uuid.md)> | IDs of items deleted offline | [optional]
**updates** | Option<[**Vec<models::SyncUpdateItem>**](SyncUpdateItem.md)> | Items updated offline | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


