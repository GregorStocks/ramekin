# ImportRecipeRequest

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**extractionMethod** | [**ImportExtractionMethod**](ImportExtractionMethod.md) | The extraction/import method used | 
**idempotencyKey** | **String** | Client-chosen key for this import. Resubmitting a key the user already used returns the original job (200) instead of creating another recipe; the resubmission&#39;s photo_ids are then ignored. | [optional] 
**photoIds** | **[UUID]** | Photo IDs that have already been uploaded via POST /api/photos | 
**rawRecipe** | [**ImportRawRecipe**](ImportRawRecipe.md) | The raw recipe data (converted from import source by client) | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


