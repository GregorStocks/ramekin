# IngredientWeightsStatus

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**estimated** | **i64** | Estimated; lines using one say \"estimated weight\". | 
**failed** | **i64** | The last attempt failed; retry to try again. | 
**failures** | [**Vec<models::IngredientWeightFailure>**](IngredientWeightFailure.md) | The most recent failures, failed re-asks included. | 
**no_typical_weight** | **i64** | The model said there's no typical weight; still unknown in estimates. | 
**pending** | **i64** | Waiting for the background estimator (queued when an estimate is shown). | 
**reask_failed** | **i64** | Re-asking with the current model failed; the earlier model's estimate is still used (and counted above). Retry to try again. | 
**reasking** | **i64** | Being asked again by the current model; the earlier model's estimate is used meanwhile (and counted above). | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


