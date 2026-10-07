# ImportRecipeRequest

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**extraction_method** | [**models::ImportExtractionMethod**](ImportExtractionMethod.md) | The extraction/import method used | 
**idempotency_key** | Option<**String**> | Client-chosen key for this import. Resubmitting a key the user already used returns the original job (200) instead of creating another recipe; the resubmission's photo_ids are then ignored. | [optional]
**photo_ids** | [**Vec<uuid::Uuid>**](uuid::Uuid.md) | Photo IDs that have already been uploaded via POST /api/photos | 
**raw_recipe** | [**models::ImportRawRecipe**](ImportRawRecipe.md) | The raw recipe data (converted from import source by client) | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


