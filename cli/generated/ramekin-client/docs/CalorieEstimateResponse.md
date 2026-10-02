# CalorieEstimateResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**database_version** | **String** | Identifies the pinned source data, aliases, and calculation rules. | 
**headline** | **String** | The main line: \"~520 kcal per serving\", \"At least ~3,100 kcal for the whole recipe\", or \"Not enough ingredient data to estimate calories\". | 
**known_calories** | Option<[**models::CalorieRange**](CalorieRange.md)> | Null when nothing was counted. A lower bound when status is partial. | [optional]
**lines** | [**Vec<models::CalorieLine>**](CalorieLine.md) | The breakdown, one entry per ingredient in order. | 
**not_counted** | **Vec<String>** | Ingredients the figures leave out, in recipe order: those given no amount (even in a complete estimate), and for a partial estimate the ones that couldn't be counted. | 
**per_serving_calories** | Option<[**models::CalorieRange**](CalorieRange.md)> |  | [optional]
**resolving** | **bool** | Some ingredient names are still being recognized in the background; ask again shortly for an estimate that includes them. | 
**secondary** | Option<**String**> | Shown under the headline when present. | [optional]
**status** | [**models::CalorieStatus**](CalorieStatus.md) |  | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


