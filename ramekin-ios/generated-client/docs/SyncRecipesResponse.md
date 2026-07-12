# SyncRecipesResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**cursor** | **Int64** | Opaque cursor to pass to the next sync. Changes may be redelivered across syncs, but none can be skipped. | 
**deleted** | **[UUID]** | Recipe IDs deleted at or after &#x60;cursor&#x60;. | 
**recipes** | [SyncRecipe] | Active recipes changed at or after &#x60;cursor&#x60;. All active recipes are returned when &#x60;cursor&#x60; is absent. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


