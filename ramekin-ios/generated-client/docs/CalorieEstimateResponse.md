# CalorieEstimateResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**databaseVersion** | **String** | Identifies the pinned source data, aliases, and calculation rules. | 
**headline** | **String** | The main line: \&quot;~520 kcal per serving\&quot;, \&quot;At least ~3,100 kcal for the whole recipe\&quot;, or \&quot;Not enough ingredient data to estimate calories\&quot;. | 
**knownCalories** | [**CalorieRange**](CalorieRange.md) | Null when nothing was counted. A lower bound when status is partial. | [optional] 
**lines** | [CalorieLine] | The breakdown, one entry per ingredient in order. | 
**notCounted** | **[String]** | Ingredients the figures leave out, in recipe order: those given no amount (even in a complete estimate), and for a partial estimate the ones that couldn&#39;t be counted. | 
**perServingCalories** | [**CalorieRange**](CalorieRange.md) |  | [optional] 
**resolving** | **Bool** | Some ingredient names are still being recognized, or weights estimated, in the background; ask again shortly for an estimate that includes them. | 
**secondary** | **String** | Shown under the headline when present. | [optional] 
**status** | [**CalorieStatus**](CalorieStatus.md) |  | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


