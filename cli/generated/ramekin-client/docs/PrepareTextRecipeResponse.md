# PrepareTextRecipeResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**content** | [**models::RecipeContent**](RecipeContent.md) |  | 
**derived_measurements** | [**Vec<models::DerivedMeasurement>**](DerivedMeasurement.md) | Approximate grams for `content.ingredients` that have them, for previewing the draft. Never saved. | 
**raw_ingredients** | **String** | Editable ingredient lines. Send these as raw_ingredients when saving. | 
**warnings** | **Vec<String>** |  | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


