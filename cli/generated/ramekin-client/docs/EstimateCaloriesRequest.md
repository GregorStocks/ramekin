# EstimateCaloriesRequest

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**ingredients** | [**Vec<models::Ingredient>**](Ingredient.md) |  | 
**scale** | **f64** | Multiplier applied to the whole recipe, including its serving count. | 
**servings** | Option<**String**> | Original, unscaled servings text: a count or range, optionally with a \"serves\", \"servings\", \"yield\", or \"makes\" prefix and a \"servings\", \"people\", \"persons\", or \"portions\" suffix (\"Serves 4 to 6\", \"Yield: 4\"). A yield of something else (\"Makes 12 cookies\") gives no per-serving figure. | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


