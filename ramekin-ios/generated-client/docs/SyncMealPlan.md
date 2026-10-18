# SyncMealPlan

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**changeSeq** | **Int64** | Sync stamp of the plan&#39;s own last change. A cached copy with a higher one is newer than this row. | 
**id** | **UUID** |  | 
**mealDate** | **Date** |  | 
**mealType** | [**MealType**](MealType.md) |  | 
**notes** | **String** |  | [optional] 
**recipeId** | **UUID** |  | 
**recipeTitle** | **String** | The recipe&#39;s current title and first photo. A new recipe version resends the plans that use it. | 
**thumbnailPhotoId** | **UUID** |  | [optional] 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


