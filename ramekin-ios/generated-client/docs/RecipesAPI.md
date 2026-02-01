# RecipesAPI

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**createRecipe**](RecipesAPI.md#createrecipe) | **POST** /api/recipes | 
[**deleteRecipe**](RecipesAPI.md#deleterecipe) | **DELETE** /api/recipes/{id} | 
[**exportAllRecipes**](RecipesAPI.md#exportallrecipes) | **GET** /api/recipes/export | 
[**exportRecipe**](RecipesAPI.md#exportrecipe) | **GET** /api/recipes/{id}/export | 
[**getRecipe**](RecipesAPI.md#getrecipe) | **GET** /api/recipes/{id} | 
[**listRecipes**](RecipesAPI.md#listrecipes) | **GET** /api/recipes | 
[**listVersions**](RecipesAPI.md#listversions) | **GET** /api/recipes/{id}/versions | 
[**rescrape**](RecipesAPI.md#rescrape) | **POST** /api/recipes/{id}/rescrape | 
[**updateRecipe**](RecipesAPI.md#updaterecipe) | **PUT** /api/recipes/{id} | 


# **createRecipe**
```swift
    open class func createRecipe(createRecipeRequest: CreateRecipeRequest, completion: @escaping (_ data: CreateRecipeResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let createRecipeRequest = CreateRecipeRequest(cookTime: "cookTime_example", description: "description_example", difficulty: "difficulty_example", ingredients: [Ingredient(item: "item_example", measurements: [Measurement(amount: "amount_example", unit: "unit_example")], note: "note_example", raw: "raw_example", section: "section_example")], instructions: "instructions_example", notes: "notes_example", nutritionalInfo: "nutritionalInfo_example", prepTime: "prepTime_example", rating: 123, servings: "servings_example", sourceName: "sourceName_example", sourceUrl: "sourceUrl_example", tags: ["tags_example"], title: "title_example", totalTime: "totalTime_example", photoIds: [123]) // CreateRecipeRequest | 

RecipesAPI.createRecipe(createRecipeRequest: createRecipeRequest) { (response, error) in
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
 **createRecipeRequest** | [**CreateRecipeRequest**](CreateRecipeRequest.md) |  | 

### Return type

[**CreateRecipeResponse**](CreateRecipeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **deleteRecipe**
```swift
    open class func deleteRecipe(id: UUID, completion: @escaping (_ data: Void?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Recipe ID

RecipesAPI.deleteRecipe(id: id) { (response, error) in
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

### Return type

Void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **exportAllRecipes**
```swift
    open class func exportAllRecipes(completion: @escaping (_ data: Void?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient


RecipesAPI.exportAllRecipes() { (response, error) in
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
This endpoint does not need any parameter.

### Return type

Void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/zip, application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **exportRecipe**
```swift
    open class func exportRecipe(id: UUID, completion: @escaping (_ data: Void?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Recipe ID

RecipesAPI.exportRecipe(id: id) { (response, error) in
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

### Return type

Void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/gzip, application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **getRecipe**
```swift
    open class func getRecipe(id: UUID, versionId: UUID? = nil, completion: @escaping (_ data: RecipeResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Recipe ID
let versionId = 987 // UUID | Optional version ID to fetch a specific version instead of current (optional)

RecipesAPI.getRecipe(id: id, versionId: versionId) { (response, error) in
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
 **versionId** | **UUID** | Optional version ID to fetch a specific version instead of current | [optional] 

### Return type

[**RecipeResponse**](RecipeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **listRecipes**
```swift
    open class func listRecipes(limit: Int64? = nil, offset: Int64? = nil, q: String? = nil, sortBy: SortBy? = nil, sortDir: Direction? = nil, completion: @escaping (_ data: ListRecipesResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let limit = 987 // Int64 | Number of items to return (default: 20, max: 1000) (optional)
let offset = 987 // Int64 | Number of items to skip (default: 0) (optional)
let q = "q_example" // String | Search query with optional filters. Supports: - Plain text: searches title and description - tag:value: filter by tag (can use multiple) - source:value: filter by source name - has:photos / no:photos: filter by photo presence - created:>2024-01-01: created after date - created:<2024-12-31: created before date - created:2024-01-01..2024-12-31: created in date range  Example: \"chicken tag:dinner tag:quick has:photos\" (optional)
let sortBy = SortBy() // SortBy | Sort field (default: updated_at) (optional)
let sortDir = Direction() // Direction | Sort direction (default: desc). Ignored when sort_by=random. (optional)

RecipesAPI.listRecipes(limit: limit, offset: offset, q: q, sortBy: sortBy, sortDir: sortDir) { (response, error) in
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
 **limit** | **Int64** | Number of items to return (default: 20, max: 1000) | [optional] 
 **offset** | **Int64** | Number of items to skip (default: 0) | [optional] 
 **q** | **String** | Search query with optional filters. Supports: - Plain text: searches title and description - tag:value: filter by tag (can use multiple) - source:value: filter by source name - has:photos / no:photos: filter by photo presence - created:&gt;2024-01-01: created after date - created:&lt;2024-12-31: created before date - created:2024-01-01..2024-12-31: created in date range  Example: \&quot;chicken tag:dinner tag:quick has:photos\&quot; | [optional] 
 **sortBy** | [**SortBy**](.md) | Sort field (default: updated_at) | [optional] 
 **sortDir** | [**Direction**](.md) | Sort direction (default: desc). Ignored when sort_by&#x3D;random. | [optional] 

### Return type

[**ListRecipesResponse**](ListRecipesResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **listVersions**
```swift
    open class func listVersions(id: UUID, completion: @escaping (_ data: VersionListResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Recipe ID

RecipesAPI.listVersions(id: id) { (response, error) in
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

### Return type

[**VersionListResponse**](VersionListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **rescrape**
```swift
    open class func rescrape(id: UUID, completion: @escaping (_ data: RescrapeResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Recipe ID

RecipesAPI.rescrape(id: id) { (response, error) in
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

### Return type

[**RescrapeResponse**](RescrapeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **updateRecipe**
```swift
    open class func updateRecipe(id: UUID, updateRecipeRequest: UpdateRecipeRequest, completion: @escaping (_ data: Void?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Recipe ID
let updateRecipeRequest = UpdateRecipeRequest(cookTime: "cookTime_example", description: "description_example", difficulty: "difficulty_example", ingredients: [Ingredient(item: "item_example", measurements: [Measurement(amount: "amount_example", unit: "unit_example")], note: "note_example", raw: "raw_example", section: "section_example")], instructions: "instructions_example", notes: "notes_example", nutritionalInfo: "nutritionalInfo_example", photoIds: [123], prepTime: "prepTime_example", rating: 123, servings: "servings_example", sourceName: "sourceName_example", sourceUrl: "sourceUrl_example", tags: ["tags_example"], title: "title_example", totalTime: "totalTime_example") // UpdateRecipeRequest | 

RecipesAPI.updateRecipe(id: id, updateRecipeRequest: updateRecipeRequest) { (response, error) in
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
 **updateRecipeRequest** | [**UpdateRecipeRequest**](UpdateRecipeRequest.md) |  | 

### Return type

Void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

