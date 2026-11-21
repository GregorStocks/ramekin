# SyncRecipe

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**calories_per_serving** | Option<**String**> | The estimate for display, as `RecipeSummary.calories_per_serving`. | [optional]
**created_at** | **String** |  | 
**description** | Option<**String**> |  | [optional]
**id** | [**uuid::Uuid**](uuid::Uuid.md) |  | 
**ingredient_match_text** | **String** | The stored ingredients JSON text, exactly as the database holds it — the exact haystack the server's bare-text search filter matches (JSON keys and syntax included). Local search must match against this string, not a re-encoding of `ingredients`, to reproduce server result membership. | 
**ingredients** | [**Vec<models::Ingredient>**](Ingredient.md) |  | 
**instructions** | **String** |  | 
**kcal_per_serving** | Option<**f64**> | Estimated calories per serving: what `calories:` filters and the calorie sort compare. Null when there's no per-serving estimate. | [optional]
**notes** | Option<**String**> |  | [optional]
**photos** | [**Vec<models::SyncPhoto>**](SyncPhoto.md) | The recipe's photos, in order, leaving out deleted ones: what `photo_size:` and `photo_dim:` filters compare. | 
**rating** | Option<**i32**> |  | [optional]
**source_name** | Option<**String**> | What `source:` filters match. | [optional]
**tags** | **Vec<String>** |  | 
**thumbnail_photo_id** | Option<[**uuid::Uuid**](uuid::Uuid.md)> |  | [optional]
**title** | **String** |  | 
**updated_at** | **String** |  | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


