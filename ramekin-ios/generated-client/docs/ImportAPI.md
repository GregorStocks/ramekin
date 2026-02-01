# ImportAPI

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**importRecipe**](ImportAPI.md#importrecipe) | **POST** /api/import/recipe | 


# **importRecipe**
```swift
    open class func importRecipe(importRecipeRequest: ImportRecipeRequest, completion: @escaping (_ data: ImportRecipeResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let importRecipeRequest = ImportRecipeRequest(extractionMethod: ImportExtractionMethod(), photoIds: [123], rawRecipe: ImportRawRecipe(categories: ["categories_example"], cookTime: "cookTime_example", description: "description_example", difficulty: "difficulty_example", imageUrls: ["imageUrls_example"], ingredients: "ingredients_example", instructions: "instructions_example", notes: "notes_example", nutritionalInfo: "nutritionalInfo_example", prepTime: "prepTime_example", rating: 123, servings: "servings_example", sourceName: "sourceName_example", sourceUrl: "sourceUrl_example", title: "title_example", totalTime: "totalTime_example")) // ImportRecipeRequest | 

ImportAPI.importRecipe(importRecipeRequest: importRecipeRequest) { (response, error) in
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
 **importRecipeRequest** | [**ImportRecipeRequest**](ImportRecipeRequest.md) |  | 

### Return type

[**ImportRecipeResponse**](ImportRecipeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

