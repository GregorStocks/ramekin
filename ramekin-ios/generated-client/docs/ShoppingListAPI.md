# ShoppingListAPI

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**clearChecked**](ShoppingListAPI.md#clearchecked) | **DELETE** /api/shopping-list/clear-checked | 
[**createItems**](ShoppingListAPI.md#createitems) | **POST** /api/shopping-list | 
[**deleteItem**](ShoppingListAPI.md#deleteitem) | **DELETE** /api/shopping-list/{id} | 
[**listItems**](ShoppingListAPI.md#listitems) | **GET** /api/shopping-list | 
[**syncItems**](ShoppingListAPI.md#syncitems) | **POST** /api/shopping-list/sync | 
[**updateItem**](ShoppingListAPI.md#updateitem) | **PUT** /api/shopping-list/{id} | 


# **clearChecked**
```swift
    open class func clearChecked(completion: @escaping (_ data: ClearCheckedResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient


ShoppingListAPI.clearChecked() { (response, error) in
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

[**ClearCheckedResponse**](ClearCheckedResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **createItems**
```swift
    open class func createItems(createShoppingListRequest: CreateShoppingListRequest, completion: @escaping (_ data: CreateShoppingListResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let createShoppingListRequest = CreateShoppingListRequest(items: [CreateShoppingListItemRequest(amount: "amount_example", clientId: 123, item: "item_example", note: "note_example", sourceRecipeId: 123, sourceRecipeTitle: "sourceRecipeTitle_example")]) // CreateShoppingListRequest | 

ShoppingListAPI.createItems(createShoppingListRequest: createShoppingListRequest) { (response, error) in
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
 **createShoppingListRequest** | [**CreateShoppingListRequest**](CreateShoppingListRequest.md) |  | 

### Return type

[**CreateShoppingListResponse**](CreateShoppingListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **deleteItem**
```swift
    open class func deleteItem(id: UUID, completion: @escaping (_ data: Void?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Shopping list item ID

ShoppingListAPI.deleteItem(id: id) { (response, error) in
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
 **id** | **UUID** | Shopping list item ID | 

### Return type

Void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **listItems**
```swift
    open class func listItems(completion: @escaping (_ data: ShoppingListResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient


ShoppingListAPI.listItems() { (response, error) in
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

[**ShoppingListResponse**](ShoppingListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **syncItems**
```swift
    open class func syncItems(syncRequest: SyncRequest, completion: @escaping (_ data: SyncResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let syncRequest = SyncRequest(creates: [SyncCreateItem(amount: "amount_example", clientId: 123, isChecked: false, item: "item_example", note: "note_example", sortOrder: 123, sourceRecipeId: 123, sourceRecipeTitle: "sourceRecipeTitle_example")], deletes: [123], lastSyncAt: Date(), updates: [SyncUpdateItem(amount: "amount_example", expectedVersion: 123, id: 123, isChecked: false, item: "item_example", note: "note_example", sortOrder: 123)]) // SyncRequest | 

ShoppingListAPI.syncItems(syncRequest: syncRequest) { (response, error) in
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
 **syncRequest** | [**SyncRequest**](SyncRequest.md) |  | 

### Return type

[**SyncResponse**](SyncResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **updateItem**
```swift
    open class func updateItem(id: UUID, updateShoppingListItemRequest: UpdateShoppingListItemRequest, completion: @escaping (_ data: Void?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Shopping list item ID
let updateShoppingListItemRequest = UpdateShoppingListItemRequest(amount: "amount_example", isChecked: false, item: "item_example", note: "note_example", sortOrder: 123) // UpdateShoppingListItemRequest | 

ShoppingListAPI.updateItem(id: id, updateShoppingListItemRequest: updateShoppingListItemRequest) { (response, error) in
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
 **id** | **UUID** | Shopping list item ID | 
 **updateShoppingListItemRequest** | [**UpdateShoppingListItemRequest**](UpdateShoppingListItemRequest.md) |  | 

### Return type

Void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

