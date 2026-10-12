# IngredientNamesStatusResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**estimated** | **Int64** | A real food no catalog entry matches, counted with the model&#39;s own calories (\&quot;estimated calories\&quot;). | 
**failed** | **Int64** | Every attempt gave an invalid answer; retry to try again. | 
**failures** | [IngredientNameFailure] | The most recent failures. | 
**notFood** | **Int64** | Resolved as not an ingredient (a heading, a serving note). | 
**pending** | **Int64** | Waiting for the background resolver (including names whose answer was invalid and will be asked again). | 
**recognized** | **Int64** | Resolved to a catalog food or product (for an ambiguous name, the one a recipe most likely means). | 
**unknown** | **Int64** | Resolved, but the model couldn&#39;t tell; still unknown in estimates. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


