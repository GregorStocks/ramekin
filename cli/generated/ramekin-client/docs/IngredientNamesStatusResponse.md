# IngredientNamesStatusResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**failed** | **i64** | The last attempt failed; retry to try again. | 
**failures** | [**Vec<models::IngredientNameFailure>**](IngredientNameFailure.md) | The most recent failures. | 
**not_food** | **i64** | Resolved as not an ingredient (a heading, a serving note). | 
**pending** | **i64** | Waiting for the background resolver. | 
**recognized** | **i64** | Resolved to a catalog food or product. | 
**unknown** | **i64** | Resolved, but the model couldn't tell; still unknown in estimates. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


