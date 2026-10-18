# \MealPlansApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**list_meal_plans**](MealPlansApi.md#list_meal_plans) | **GET** /api/meal-plans | 



## list_meal_plans

> models::MealPlanListResponse list_meal_plans(start_date, end_date)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**start_date** | Option<**String**> | Start date (inclusive), format: YYYY-MM-DD. Defaults to today. |  |
**end_date** | Option<**String**> | End date (inclusive), format: YYYY-MM-DD. Defaults to start_date + 6 days (one week). |  |

### Return type

[**models::MealPlanListResponse**](MealPlanListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

