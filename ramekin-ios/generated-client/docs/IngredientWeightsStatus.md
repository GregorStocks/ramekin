# IngredientWeightsStatus

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**estimated** | **Int64** | Estimated; lines using one say \&quot;estimated weight\&quot;. | 
**failed** | **Int64** | The last attempt failed; retry to try again. | 
**failures** | [IngredientWeightFailure] | The most recent failures. | 
**noTypicalWeight** | **Int64** | The model said there&#39;s no typical weight; still unknown in estimates. | 
**pending** | **Int64** | Waiting for the background estimator (queued when an estimate is shown). | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


