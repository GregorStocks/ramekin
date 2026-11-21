# SyncRecipe

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**caloriesPerServing** | **String** | The estimate for display, as &#x60;RecipeSummary.calories_per_serving&#x60;. | [optional] 
**createdAt** | **Date** |  | 
**description** | **String** |  | [optional] 
**id** | **UUID** |  | 
**ingredientMatchText** | **String** | The stored ingredients JSON text, exactly as the database holds it — the exact haystack the server&#39;s bare-text search filter matches (JSON keys and syntax included). Local search must match against this string, not a re-encoding of &#x60;ingredients&#x60;, to reproduce server result membership. | 
**ingredients** | [Ingredient] |  | 
**instructions** | **String** |  | 
**kcalPerServing** | **Double** | Estimated calories per serving: what &#x60;calories:&#x60; filters and the calorie sort compare. Null when there&#39;s no per-serving estimate. | [optional] 
**notes** | **String** |  | [optional] 
**photos** | [SyncPhoto] | The recipe&#39;s photos, in order, leaving out deleted ones: what &#x60;photo_size:&#x60; and &#x60;photo_dim:&#x60; filters compare. | 
**rating** | **Int** |  | [optional] 
**sourceName** | **String** | What &#x60;source:&#x60; filters match. | [optional] 
**tags** | **[String]** |  | 
**thumbnailPhotoId** | **UUID** |  | [optional] 
**title** | **String** |  | 
**updatedAt** | **Date** |  | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


