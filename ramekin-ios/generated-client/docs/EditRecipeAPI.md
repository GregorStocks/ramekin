# EditRecipeAPI

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**editRecipeWithAi**](EditRecipeAPI.md#editrecipewithai) | **POST** /api/edit-recipe | Edit a recipe with AI


# **editRecipeWithAi**
```swift
    open class func editRecipeWithAi(editRecipeRequest: EditRecipeRequest, completion: @escaping (_ data: RecipeContent?, _ error: Error?) -> Void)
```

Edit a recipe with AI

Takes a recipe and a free-text instruction describing the desired change. Returns the complete modified recipe. Stateless - does NOT modify any database records.

### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let editRecipeRequest = EditRecipeRequest(instruction: "instruction_example", photoIds: [123], recipe: RecipeContent(cookTime: "cookTime_example", description: "description_example", difficulty: "difficulty_example", ingredients: [Ingredient(item: "item_example", measurements: [Measurement(amount: "amount_example", unit: "unit_example")], note: "note_example", section: "section_example")], instructions: "instructions_example", notes: "notes_example", nutritionalInfo: "nutritionalInfo_example", prepTime: "prepTime_example", rating: 123, servings: "servings_example", sourceName: "sourceName_example", sourceUrl: "sourceUrl_example", tags: ["tags_example"], title: "title_example", totalTime: "totalTime_example")) // EditRecipeRequest | 

// Edit a recipe with AI
EditRecipeAPI.editRecipeWithAi(editRecipeRequest: editRecipeRequest) { (response, error) in
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
 **editRecipeRequest** | [**EditRecipeRequest**](EditRecipeRequest.md) |  | 

### Return type

[**RecipeContent**](RecipeContent.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

