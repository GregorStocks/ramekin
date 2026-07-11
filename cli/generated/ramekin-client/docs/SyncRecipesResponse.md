# SyncRecipesResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**deleted** | [**Vec<uuid::Uuid>**](uuid::Uuid.md) | Recipe IDs deleted since last_sync_at. | 
**recipes** | [**Vec<models::SyncRecipe>**](SyncRecipe.md) | Active recipes created or updated since last_sync_at. All active recipes are returned when last_sync_at is absent. | 
**sync_timestamp** | **String** | New sync timestamp to use for the next sync. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


