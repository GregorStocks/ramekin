# CalorieEstimateResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**database_version** | **String** | Identifies the pinned source data, aliases, and calculation rules. | 
**headline** | **String** | The main line: \"~520 kcal per serving\", \"At least ~3,100 kcal for the whole recipe\", or \"Not enough ingredient data to estimate calories\". | 
**known_calories** | Option<[**models::CalorieRange**](CalorieRange.md)> | Null when nothing was counted. A lower bound when status is partial. | [optional]
**lines** | [**Vec<models::CalorieLine>**](CalorieLine.md) | The breakdown, one entry per ingredient in order. | 
**not_counted** | **Vec<String>** | For a partial estimate, the ingredients its lower bound leaves out. | 
**per_serving_calories** | Option<[**models::CalorieRange**](CalorieRange.md)> |  | [optional]
**resolving** | **bool** | Some ingredient names are still being recognized in the background; ask again shortly for an estimate that includes them. | 
**secondary** | Option<**String**> | Shown under the headline when present. | [optional]
**status** | [**models::CalorieStatus**](CalorieStatus.md) |  | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


