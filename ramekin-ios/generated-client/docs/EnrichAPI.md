# EnrichAPI

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**customEnrichRecipe**](EnrichAPI.md#customenrichrecipe) | **POST** /api/enrich/custom | Apply a custom AI modification to a recipe
[**enrichRecipe**](EnrichAPI.md#enrichrecipe) | **POST** /api/enrich | Enrich a recipe


# **customEnrichRecipe**
```swift
    open class func customEnrichRecipe(customEnrichRequest: CustomEnrichRequest, completion: @escaping (_ data: RecipeContent?, _ error: Error?) -> Void)
```

Apply a custom AI modification to a recipe

Takes a recipe and a free-text instruction describing the desired change. Returns the complete modified recipe. Stateless - does NOT modify any database records.

### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let customEnrichRequest = CustomEnrichRequest(instruction: "instruction_example", recipe: RecipeContent(cookTime: "cookTime_example", description: "description_example", difficulty: "difficulty_example", ingredients: [Ingredient(item: "item_example", measurements: [Measurement(amount: "amount_example", unit: "unit_example")], note: "note_example", raw: "raw_example", section: "section_example")], instructions: "instructions_example", notes: "notes_example", nutritionalInfo: "nutritionalInfo_example", prepTime: "prepTime_example", rating: 123, servings: "servings_example", sourceName: "sourceName_example", sourceUrl: "sourceUrl_example", tags: ["tags_example"], title: "title_example", totalTime: "totalTime_example")) // CustomEnrichRequest | 

// Apply a custom AI modification to a recipe
EnrichAPI.customEnrichRecipe(customEnrichRequest: customEnrichRequest) { (response, error) in
    guard error == nil else {
        print(error)
        return
    }

    if (response) {
        dump(response)
    }
}
```

### Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **customEnrichRequest** | [**CustomEnrichRequest**](CustomEnrichRequest.md) |  | 

### Return type

[**RecipeContent**](RecipeContent.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **enrichRecipe**
```swift
    open class func enrichRecipe(recipeContent: RecipeContent, completion: @escaping (_ data: RecipeContent?, _ error: Error?) -> Void)
```

Enrich a recipe

This is a stateless endpoint that takes a recipe object and returns an enriched version. It does NOT modify any database records. The client can apply the enriched data via a normal PUT /api/recipes/{id} call.  Enriches: - Ingredient measurements with gram conversions (volume/weight → grams) - Tags by suggesting from the user's existing tag library (requires AI; skipped if unavailable)

### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let recipeContent = RecipeContent(cookTime: "cookTime_example", description: "description_example", difficulty: "difficulty_example", ingredients: [Ingredient(item: "item_example", measurements: [Measurement(amount: "amount_example", unit: "unit_example")], note: "note_example", raw: "raw_example", section: "section_example")], instructions: "instructions_example", notes: "notes_example", nutritionalInfo: "nutritionalInfo_example", prepTime: "prepTime_example", rating: 123, servings: "servings_example", sourceName: "sourceName_example", sourceUrl: "sourceUrl_example", tags: ["tags_example"], title: "title_example", totalTime: "totalTime_example") // RecipeContent | 

// Enrich a recipe
EnrichAPI.enrichRecipe(recipeContent: recipeContent) { (response, error) in
    guard error == nil else {
        print(error)
        return
    }

    if (response) {
        dump(response)
    }
}
```

### Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **recipeContent** | [**RecipeContent**](RecipeContent.md) |  | 

### Return type

[**RecipeContent**](RecipeContent.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

