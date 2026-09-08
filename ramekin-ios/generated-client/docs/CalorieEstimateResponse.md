# CalorieEstimateResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**databaseVersion** | **String** | Identifies the pinned source data, aliases, and calculation rules. | 
**knownCalories** | [**CalorieRange**](CalorieRange.md) | Null when no ingredient could be estimated. Otherwise a subtotal that may be partial. | [optional] 
**perServingCalories** | [**CalorieRange**](CalorieRange.md) |  | [optional] 
**perServingSummary** | **String** |  | [optional] 
**summary** | **String** |  | 
**unknownIngredients** | [UnknownCalorieIngredient] |  | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


