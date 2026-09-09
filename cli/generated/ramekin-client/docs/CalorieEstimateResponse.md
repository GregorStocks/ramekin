# CalorieEstimateResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**database_version** | **String** | Identifies the pinned source data, aliases, and calculation rules. | 
**known_calories** | Option<[**models::CalorieRange**](CalorieRange.md)> | Null when no ingredient could be estimated. Otherwise a subtotal that may be partial. | [optional]
**per_serving_calories** | Option<[**models::CalorieRange**](CalorieRange.md)> |  | [optional]
**per_serving_summary** | Option<**String**> |  | [optional]
**summary** | **String** |  | 
**unknown_ingredients** | [**Vec<models::UnknownCalorieIngredient>**](UnknownCalorieIngredient.md) |  | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


