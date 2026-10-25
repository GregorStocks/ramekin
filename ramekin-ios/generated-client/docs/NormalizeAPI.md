# NormalizeAPI

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**acceptNormalizeProposal**](NormalizeAPI.md#acceptnormalizeproposal) | **POST** /api/recipes/{id}/normalize/{job_id}/accept | 
[**getNormalizeProposal**](NormalizeAPI.md#getnormalizeproposal) | **GET** /api/recipes/{id}/normalize/{job_id} | 
[**listNormalizeOps**](NormalizeAPI.md#listnormalizeops) | **GET** /api/recipes/normalize-ops | 
[**normalizeRecipe**](NormalizeAPI.md#normalizerecipe) | **POST** /api/recipes/{id}/normalize | 
[**normalizeRecipes**](NormalizeAPI.md#normalizerecipes) | **POST** /api/recipes/normalize | 


# **acceptNormalizeProposal**
```swift
    open class func acceptNormalizeProposal(id: UUID, jobId: UUID, completion: @escaping (_ data: AcceptNormalizeResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Recipe ID
let jobId = 987 // UUID | Normalize job ID

NormalizeAPI.acceptNormalizeProposal(id: id, jobId: jobId) { (response, error) in
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
 **jobId** | **UUID** | Normalize job ID | 

### Return type

[**AcceptNormalizeResponse**](AcceptNormalizeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **getNormalizeProposal**
```swift
    open class func getNormalizeProposal(id: UUID, jobId: UUID, completion: @escaping (_ data: NormalizeProposalResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Recipe ID
let jobId = 987 // UUID | Normalize job ID

NormalizeAPI.getNormalizeProposal(id: id, jobId: jobId) { (response, error) in
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
 **jobId** | **UUID** | Normalize job ID | 

### Return type

[**NormalizeProposalResponse**](NormalizeProposalResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **listNormalizeOps**
```swift
    open class func listNormalizeOps(recipeId: UUID? = nil, completion: @escaping (_ data: NormalizeOpsResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let recipeId = 987 // UUID | Report which ops can run on this recipe. (optional)

NormalizeAPI.listNormalizeOps(recipeId: recipeId) { (response, error) in
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
 **recipeId** | **UUID** | Report which ops can run on this recipe. | [optional] 

### Return type

[**NormalizeOpsResponse**](NormalizeOpsResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **normalizeRecipe**
```swift
    open class func normalizeRecipe(id: UUID, normalizeRequest: NormalizeRequest, completion: @escaping (_ data: NormalizeJobResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Recipe ID
let normalizeRequest = NormalizeRequest(ops: ["ops_example"]) // NormalizeRequest | 

NormalizeAPI.normalizeRecipe(id: id, normalizeRequest: normalizeRequest) { (response, error) in
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
 **normalizeRequest** | [**NormalizeRequest**](NormalizeRequest.md) |  | 

### Return type

[**NormalizeJobResponse**](NormalizeJobResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **normalizeRecipes**
```swift
    open class func normalizeRecipes(bulkNormalizeRequest: BulkNormalizeRequest, completion: @escaping (_ data: BulkNormalizeResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let bulkNormalizeRequest = BulkNormalizeRequest(ops: ["ops_example"], recipeIds: [123]) // BulkNormalizeRequest | 

NormalizeAPI.normalizeRecipes(bulkNormalizeRequest: bulkNormalizeRequest) { (response, error) in
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
 **bulkNormalizeRequest** | [**BulkNormalizeRequest**](BulkNormalizeRequest.md) |  | 

### Return type

[**BulkNormalizeResponse**](BulkNormalizeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

