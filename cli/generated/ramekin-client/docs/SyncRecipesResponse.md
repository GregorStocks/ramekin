# SyncRecipesResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**cursor** | **i64** | Opaque cursor to pass to the next sync. Changes may be redelivered across syncs, but none can be skipped. | 
**deleted** | [**Vec<uuid::Uuid>**](uuid::Uuid.md) | Recipe IDs deleted at or after `cursor`. | 
**recipes** | [**Vec<models::SyncRecipe>**](SyncRecipe.md) | Active recipes changed at or after `cursor`. All active recipes are returned when `cursor` is absent. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


