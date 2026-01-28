# ImportRecipeRequest

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**extraction_method** | [**models::ImportExtractionMethod**](ImportExtractionMethod.md) | The extraction/import method used | 
**photo_ids** | [**Vec<uuid::Uuid>**](uuid::Uuid.md) | Photo IDs that have already been uploaded via POST /api/photos | 
**raw_recipe** | [**models::ImportRawRecipe**](ImportRawRecipe.md) | The raw recipe data (converted from import source by client) | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


