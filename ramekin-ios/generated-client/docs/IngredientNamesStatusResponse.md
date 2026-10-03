# IngredientNamesStatusResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**failed** | **Int64** | The last attempt failed; retry to try again. | 
**failures** | [IngredientNameFailure] | The most recent failures. | 
**notFood** | **Int64** | Resolved as not an ingredient (a heading, a serving note). | 
**pending** | **Int64** | Waiting for the background resolver. | 
**recognized** | **Int64** | Resolved to a catalog food or product. | 
**unknown** | **Int64** | Resolved, but the model couldn&#39;t tell; still unknown in estimates. | 
**weights** | [**IngredientWeightsStatus**](IngredientWeightsStatus.md) |  | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


