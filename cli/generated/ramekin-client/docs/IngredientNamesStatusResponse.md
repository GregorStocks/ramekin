# IngredientNamesStatusResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**estimated** | **i64** | A real food no catalog entry matches, counted with the model's own calories (\"estimated calories\"). | 
**failed** | **i64** | Every attempt gave an invalid answer; retry to try again. | 
**failures** | [**Vec<models::IngredientNameFailure>**](IngredientNameFailure.md) | The most recent failures. | 
**not_food** | **i64** | Resolved as not an ingredient (a heading, a serving note). | 
**pending** | **i64** | Waiting for the background resolver (including names whose answer was invalid and will be asked again). | 
**recognized** | **i64** | Resolved to a catalog food or product (for an ambiguous name, the one a recipe most likely means). | 
**unknown** | **i64** | Resolved, but the model couldn't tell; still unknown in estimates. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


