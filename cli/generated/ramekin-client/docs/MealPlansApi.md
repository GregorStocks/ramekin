# \MealPlansApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_meal_plan**](MealPlansApi.md#create_meal_plan) | **POST** /api/meal-plans | 
[**delete_meal_plan**](MealPlansApi.md#delete_meal_plan) | **DELETE** /api/meal-plans/{id} | 
[**list_meal_plans**](MealPlansApi.md#list_meal_plans) | **GET** /api/meal-plans | 
[**update_meal_plan**](MealPlansApi.md#update_meal_plan) | **PUT** /api/meal-plans/{id} | 



## create_meal_plan

> models::CreateMealPlanResponse create_meal_plan(create_meal_plan_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**create_meal_plan_request** | [**CreateMealPlanRequest**](CreateMealPlanRequest.md) |  | [required] |

### Return type

[**models::CreateMealPlanResponse**](CreateMealPlanResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## delete_meal_plan

> delete_meal_plan(id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Meal plan ID | [required] |

### Return type

 (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


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


## update_meal_plan

> update_meal_plan(id, update_meal_plan_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Meal plan ID | [required] |
**update_meal_plan_request** | [**UpdateMealPlanRequest**](UpdateMealPlanRequest.md) |  | [required] |

### Return type

 (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

