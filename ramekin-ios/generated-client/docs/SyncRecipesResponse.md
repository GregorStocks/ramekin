# SyncRecipesResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**deleted** | **[UUID]** | Recipe IDs deleted since last_sync_at. | 
**recipes** | [SyncRecipe] | Active recipes created or updated since last_sync_at. All active recipes are returned when last_sync_at is absent. | 
**syncTimestamp** | **Date** | New sync timestamp to use for the next sync. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


