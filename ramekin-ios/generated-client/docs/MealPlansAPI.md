# MealPlansAPI

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**createMealPlan**](MealPlansAPI.md#createmealplan) | **POST** /api/meal-plans | 
[**deleteMealPlan**](MealPlansAPI.md#deletemealplan) | **DELETE** /api/meal-plans/{id} | 
[**listMealPlans**](MealPlansAPI.md#listmealplans) | **GET** /api/meal-plans | 
[**updateMealPlan**](MealPlansAPI.md#updatemealplan) | **PUT** /api/meal-plans/{id} | 


# **createMealPlan**
```swift
    open class func createMealPlan(createMealPlanRequest: CreateMealPlanRequest, completion: @escaping (_ data: CreateMealPlanResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let createMealPlanRequest = CreateMealPlanRequest(mealDate: Date(), mealType: MealType(), notes: "notes_example", recipeId: 123) // CreateMealPlanRequest | 

MealPlansAPI.createMealPlan(createMealPlanRequest: createMealPlanRequest) { (response, error) in
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
 **createMealPlanRequest** | [**CreateMealPlanRequest**](CreateMealPlanRequest.md) |  | 

### Return type

[**CreateMealPlanResponse**](CreateMealPlanResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **deleteMealPlan**
```swift
    open class func deleteMealPlan(id: UUID, completion: @escaping (_ data: Void?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Meal plan ID

MealPlansAPI.deleteMealPlan(id: id) { (response, error) in
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
 **id** | **UUID** | Meal plan ID | 

### Return type

Void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **listMealPlans**
```swift
    open class func listMealPlans(startDate: Date? = nil, endDate: Date? = nil, completion: @escaping (_ data: MealPlanListResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let startDate = Date() // Date | Start date (inclusive), format: YYYY-MM-DD. Defaults to today. (optional)
let endDate = Date() // Date | End date (inclusive), format: YYYY-MM-DD. Defaults to start_date + 6 days (one week). (optional)

MealPlansAPI.listMealPlans(startDate: startDate, endDate: endDate) { (response, error) in
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
 **startDate** | **Date** | Start date (inclusive), format: YYYY-MM-DD. Defaults to today. | [optional] 
 **endDate** | **Date** | End date (inclusive), format: YYYY-MM-DD. Defaults to start_date + 6 days (one week). | [optional] 

### Return type

[**MealPlanListResponse**](MealPlanListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **updateMealPlan**
```swift
    open class func updateMealPlan(id: UUID, updateMealPlanRequest: UpdateMealPlanRequest, completion: @escaping (_ data: Void?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Meal plan ID
let updateMealPlanRequest = UpdateMealPlanRequest(mealDate: Date(), mealType: MealType(), notes: "notes_example") // UpdateMealPlanRequest | 

MealPlansAPI.updateMealPlan(id: id, updateMealPlanRequest: updateMealPlanRequest) { (response, error) in
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
 **id** | **UUID** | Meal plan ID | 
 **updateMealPlanRequest** | [**UpdateMealPlanRequest**](UpdateMealPlanRequest.md) |  | 

### Return type

Void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

