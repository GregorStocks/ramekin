# ShoppingListAPI

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**listShoppingListHistory**](ShoppingListAPI.md#listshoppinglisthistory) | **GET** /api/shopping-list/history | 
[**undoShoppingListChange**](ShoppingListAPI.md#undoshoppinglistchange) | **POST** /api/shopping-list/history/{id}/undo | 


# **listShoppingListHistory**
```swift
    open class func listShoppingListHistory(limit: Int64? = nil, completion: @escaping (_ data: ShoppingListHistoryResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let limit = 987 // Int64 | Most entries to return, between 1 and 200. Defaults to 50. (optional)

ShoppingListAPI.listShoppingListHistory(limit: limit) { (response, error) in
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
 **limit** | **Int64** | Most entries to return, between 1 and 200. Defaults to 50. | [optional] 

### Return type

[**ShoppingListHistoryResponse**](ShoppingListHistoryResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **undoShoppingListChange**
```swift
    open class func undoShoppingListChange(id: UUID, completion: @escaping (_ data: ShoppingListHistoryEntry?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | History entry ID

ShoppingListAPI.undoShoppingListChange(id: id) { (response, error) in
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
 **id** | **UUID** | History entry ID | 

### Return type

[**ShoppingListHistoryEntry**](ShoppingListHistoryEntry.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

