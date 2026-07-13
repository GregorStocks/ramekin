# SyncRecipe

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**created_at** | **String** |  | 
**description** | Option<**String**> |  | [optional]
**id** | [**uuid::Uuid**](uuid::Uuid.md) |  | 
**ingredient_match_text** | **String** | The database's text rendering of the stored ingredients JSONB — the exact haystack the server's bare-text search filter matches (JSON keys and syntax included). Local search must match against this string, not a re-encoding of `ingredients`, to reproduce server result membership. | 
**ingredients** | [**Vec<models::Ingredient>**](Ingredient.md) |  | 
**instructions** | **String** |  | 
**notes** | Option<**String**> |  | [optional]
**rating** | Option<**i32**> |  | [optional]
**tags** | **Vec<String>** |  | 
**thumbnail_photo_id** | Option<[**uuid::Uuid**](uuid::Uuid.md)> |  | [optional]
**title** | **String** |  | 
**updated_at** | **String** |  | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


