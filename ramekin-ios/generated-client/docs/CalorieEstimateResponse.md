# CalorieEstimateResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**databaseVersion** | **String** | Identifies the pinned source data, aliases, and calculation rules. | 
**headline** | **String** | The main line: \&quot;~520 kcal per serving\&quot;, \&quot;At least ~3,100 kcal for the whole recipe\&quot;, or \&quot;Not enough ingredient data to estimate calories\&quot;. | 
**knownCalories** | [**CalorieRange**](CalorieRange.md) | Null when nothing was counted. A lower bound when status is partial. | [optional] 
**lines** | [CalorieLine] | The breakdown, one entry per ingredient in order. | 
**notCounted** | **[String]** | For a partial estimate, the ingredients its lower bound leaves out. | 
**perServingCalories** | [**CalorieRange**](CalorieRange.md) |  | [optional] 
**secondary** | **String** | Shown under the headline when present. | [optional] 
**status** | [**CalorieStatus**](CalorieStatus.md) |  | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


