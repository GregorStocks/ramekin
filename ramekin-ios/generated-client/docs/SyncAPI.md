# SyncAPI

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**syncMealPlans**](SyncAPI.md#syncmealplans) | **POST** /api/sync/meal-plans | 
[**syncRecipes**](SyncAPI.md#syncrecipes) | **GET** /api/sync/recipes | 
[**syncShoppingList**](SyncAPI.md#syncshoppinglist) | **POST** /api/sync/shopping-list | 
[**syncTags**](SyncAPI.md#synctags) | **GET** /api/sync/tags | 


# **syncMealPlans**
```swift
    open class func syncMealPlans(syncMealPlansRequest: SyncMealPlansRequest, completion: @escaping (_ data: SyncMealPlansResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let syncMealPlansRequest = SyncMealPlansRequest(cursor: 123, ops: [MealPlanOp(fields: MealPlanFields(mealDate: Date(), mealType: MealType(), notes: "notes_example", recipeId: 123), id: 123, op: OpKind())]) // SyncMealPlansRequest | 

SyncAPI.syncMealPlans(syncMealPlansRequest: syncMealPlansRequest) { (response, error) in
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
 **syncMealPlansRequest** | [**SyncMealPlansRequest**](SyncMealPlansRequest.md) |  | 

### Return type

[**SyncMealPlansResponse**](SyncMealPlansResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **syncRecipes**
```swift
    open class func syncRecipes(limit: Int64, cursor: Int64? = nil, afterId: UUID? = nil, completion: @escaping (_ data: SyncRecipesResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let limit = 987 // Int64 | Maximum number of recipes returned per page, between 1 and 500.
let cursor = 987 // Int64 | Cursor returned by a previous sync. Absent means a full sync. Every page of one sweep passes the same cursor. (optional)
let afterId = 987 // UUID | ID of the last recipe on the previous page. Absent on a sweep's first page. (optional)

SyncAPI.syncRecipes(limit: limit, cursor: cursor, afterId: afterId) { (response, error) in
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
 **limit** | **Int64** | Maximum number of recipes returned per page, between 1 and 500. | 
 **cursor** | **Int64** | Cursor returned by a previous sync. Absent means a full sync. Every page of one sweep passes the same cursor. | [optional] 
 **afterId** | **UUID** | ID of the last recipe on the previous page. Absent on a sweep&#39;s first page. | [optional] 

### Return type

[**SyncRecipesResponse**](SyncRecipesResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **syncShoppingList**
```swift
    open class func syncShoppingList(syncShoppingListRequest: SyncShoppingListRequest, completion: @escaping (_ data: SyncShoppingListResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let syncShoppingListRequest = SyncShoppingListRequest(cursor: 123, ops: [ShoppingListOp(fields: ShoppingListFields(amount: "amount_example", categoryOverride: "categoryOverride_example", isChecked: false, item: "item_example", note: "note_example", sortOrder: 123, sourceRecipeId: 123, sourceRecipeTitle: "sourceRecipeTitle_example"), id: 123, op: OpKind())]) // SyncShoppingListRequest | 

SyncAPI.syncShoppingList(syncShoppingListRequest: syncShoppingListRequest) { (response, error) in
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
 **syncShoppingListRequest** | [**SyncShoppingListRequest**](SyncShoppingListRequest.md) |  | 

### Return type

[**SyncShoppingListResponse**](SyncShoppingListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **syncTags**
```swift
    open class func syncTags(cursor: Int64? = nil, completion: @escaping (_ data: SyncTagsResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let cursor = 987 // Int64 | Cursor returned by the previous sync. Absent means a full sync. (optional)

SyncAPI.syncTags(cursor: cursor) { (response, error) in
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
 **cursor** | **Int64** | Cursor returned by the previous sync. Absent means a full sync. | [optional] 

### Return type

[**SyncTagsResponse**](SyncTagsResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

