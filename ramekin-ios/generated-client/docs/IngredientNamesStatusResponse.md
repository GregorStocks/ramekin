# IngredientNamesStatusResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**estimated** | **Int64** | A real food no catalog entry matches, counted with the model&#39;s own calories (\&quot;estimated calories\&quot;). | 
**failed** | **Int64** | The last attempt failed; retry to try again. | 
**failures** | [IngredientNameFailure] | The most recent failures, failed re-asks included. | 
**notFood** | **Int64** | Resolved as not an ingredient (a heading, a serving note). | 
**pending** | **Int64** | Waiting for the background resolver. | 
**reaskFailed** | **Int64** | Re-asking with the current model failed; the earlier model&#39;s answer is still used (and counted above). Retry to try again. | 
**recognized** | **Int64** | Resolved to a catalog food or product (for an ambiguous name, the one a recipe most likely means). | 
**unknown** | **Int64** | Resolved, but the model couldn&#39;t tell; still unknown in estimates. | 
**weights** | [**IngredientWeightsStatus**](IngredientWeightsStatus.md) |  | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


