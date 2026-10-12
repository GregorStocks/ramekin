# IngredientWeightsStatus

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**estimated** | **i64** | Estimated; lines using one say \"estimated weight\". | 
**failed** | **i64** | The last attempt failed; retry to try again. | 
**failures** | [**Vec<models::IngredientWeightFailure>**](IngredientWeightFailure.md) | The most recent failures. | 
**no_typical_weight** | **i64** | The model said there's no typical weight; still unknown in estimates. | 
**pending** | **i64** | Waiting for the background estimator (queued when an estimate is shown). | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


