# IngredientNamesAPI

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**getIngredientNamesStatus**](IngredientNamesAPI.md#getingredientnamesstatus) | **GET** /api/ingredient-names/status | 
[**retryIngredientNames**](IngredientNamesAPI.md#retryingredientnames) | **POST** /api/ingredient-names/retry | 
[**warmIngredientNames**](IngredientNamesAPI.md#warmingredientnames) | **POST** /api/ingredient-names/warm | Queue every name the catalog doesn&#39;t know from the caller&#39;s current recipes and shopping list, e.g. once after this feature ships.


# **getIngredientNamesStatus**
```swift
    open class func getIngredientNamesStatus(completion: @escaping (_ data: IngredientNamesStatusResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient


IngredientNamesAPI.getIngredientNamesStatus() { (response, error) in
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

[**IngredientNamesStatusResponse**](IngredientNamesStatusResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **retryIngredientNames**
```swift
    open class func retryIngredientNames(completion: @escaping (_ data: IngredientNamesQueuedResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient


IngredientNamesAPI.retryIngredientNames() { (response, error) in
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

[**IngredientNamesQueuedResponse**](IngredientNamesQueuedResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **warmIngredientNames**
```swift
    open class func warmIngredientNames(completion: @escaping (_ data: IngredientNamesQueuedResponse?, _ error: Error?) -> Void)
```

Queue every name the catalog doesn't know from the caller's current recipes and shopping list, e.g. once after this feature ships.

### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient


// Queue every name the catalog doesn't know from the caller's current recipes and shopping list, e.g. once after this feature ships.
IngredientNamesAPI.warmIngredientNames() { (response, error) in
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

[**IngredientNamesQueuedResponse**](IngredientNamesQueuedResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

