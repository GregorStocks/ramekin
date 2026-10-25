# EditRecipeAPI

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**editRecipeWithAi**](EditRecipeAPI.md#editrecipewithai) | **POST** /api/recipes/{id}/edit-with-ai | Edit a recipe with AI


# **editRecipeWithAi**
```swift
    open class func editRecipeWithAi(id: UUID, editRecipeRequest: EditRecipeRequest, completion: @escaping (_ data: NormalizeJobResponse?, _ error: Error?) -> Void)
```

Edit a recipe with AI

Starts a job that edits the recipe's current version the way the instruction says, looking up catalog weights and calories and the user's tags and recipes as it goes. Poll it with `GET /api/scrape/{id}`; once it completes, `GET /api/recipes/{id}/normalize/{job_id}` returns the proposed version and a summary of the changes, and `POST /api/recipes/{id}/normalize/{job_id}/accept` saves it.

### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Recipe ID
let editRecipeRequest = EditRecipeRequest(instruction: "instruction_example", photoIds: [123]) // EditRecipeRequest | 

// Edit a recipe with AI
EditRecipeAPI.editRecipeWithAi(id: id, editRecipeRequest: editRecipeRequest) { (response, error) in
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
 **id** | **UUID** | Recipe ID | 
 **editRecipeRequest** | [**EditRecipeRequest**](EditRecipeRequest.md) |  | 

### Return type

[**NormalizeJobResponse**](NormalizeJobResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

