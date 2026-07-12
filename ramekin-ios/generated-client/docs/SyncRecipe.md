# SyncRecipe

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**createdAt** | **Date** |  | 
**description** | **String** |  | [optional] 
**id** | **UUID** |  | 
**ingredientMatchText** | **String** | The database&#39;s text rendering of the stored ingredients JSONB — the exact haystack the server&#39;s bare-text search filter matches (JSON keys and syntax included). Local search must match against this string, not a re-encoding of &#x60;ingredients&#x60;, to reproduce server result membership. | 
**ingredients** | [Ingredient] |  | 
**instructions** | **String** |  | 
**notes** | **String** |  | [optional] 
**rating** | **Int** |  | [optional] 
**tags** | **[String]** |  | 
**thumbnailPhotoId** | **UUID** |  | [optional] 
**title** | **String** |  | 
**updatedAt** | **Date** |  | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


