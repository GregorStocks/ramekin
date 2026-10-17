# MealPlansAPI

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**listMealPlans**](MealPlansAPI.md#listmealplans) | **GET** /api/meal-plans | 


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

