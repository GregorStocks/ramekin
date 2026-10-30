# RecipeSummary

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**calories_per_serving** | Option<**String**> | Estimated calories per serving, for display: \"~520 kcal\", \"~380–560 kcal\", or \"~520+ kcal\" when some ingredients couldn't be counted. Null when there's no per-serving estimate. | [optional]
**created_at** | **String** |  | 
**description** | Option<**String**> |  | [optional]
**id** | [**uuid::Uuid**](uuid::Uuid.md) |  | 
**rating** | Option<**i32**> | Rating from 1-5, if set | [optional]
**tags** | **Vec<String>** |  | 
**thumbnail_photo_id** | Option<[**uuid::Uuid**](uuid::Uuid.md)> | Photo ID of the first photo (thumbnail), if any | [optional]
**title** | **String** |  | 
**updated_at** | **String** |  | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


