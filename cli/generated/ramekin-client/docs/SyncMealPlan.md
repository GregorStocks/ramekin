# SyncMealPlan

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**change_seq** | **i64** | Sync stamp of the plan's own last change. A cached copy with a higher one is newer than this row. | 
**id** | [**uuid::Uuid**](uuid::Uuid.md) |  | 
**meal_date** | [**String**](string.md) |  | 
**meal_type** | [**models::MealType**](MealType.md) |  | 
**notes** | Option<**String**> |  | [optional]
**recipe_id** | [**uuid::Uuid**](uuid::Uuid.md) |  | 
**recipe_title** | **String** | The recipe's current title and first photo. A new recipe version resends the plans that use it. | 
**thumbnail_photo_id** | Option<[**uuid::Uuid**](uuid::Uuid.md)> |  | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


