# \SyncApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**sync_meal_plans**](SyncApi.md#sync_meal_plans) | **POST** /api/sync/meal-plans | 
[**sync_recipes**](SyncApi.md#sync_recipes) | **GET** /api/sync/recipes | 
[**sync_shopping_list**](SyncApi.md#sync_shopping_list) | **POST** /api/sync/shopping-list | 
[**sync_tags**](SyncApi.md#sync_tags) | **GET** /api/sync/tags | 



## sync_meal_plans

> models::SyncMealPlansResponse sync_meal_plans(sync_meal_plans_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**sync_meal_plans_request** | [**SyncMealPlansRequest**](SyncMealPlansRequest.md) |  | [required] |

### Return type

[**models::SyncMealPlansResponse**](SyncMealPlansResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## sync_recipes

> models::SyncRecipesResponse sync_recipes(limit, cursor, after_id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**limit** | **i64** | Maximum number of recipes returned per page, between 1 and 500. | [required] |
**cursor** | Option<**i64**> | Cursor returned by a previous sync. Absent means a full sync. Every page of one sweep passes the same cursor. |  |
**after_id** | Option<**uuid::Uuid**> | ID of the last recipe on the previous page. Absent on a sweep's first page. |  |

### Return type

[**models::SyncRecipesResponse**](SyncRecipesResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## sync_shopping_list

> models::SyncShoppingListResponse sync_shopping_list(sync_shopping_list_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**sync_shopping_list_request** | [**SyncShoppingListRequest**](SyncShoppingListRequest.md) |  | [required] |

### Return type

[**models::SyncShoppingListResponse**](SyncShoppingListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## sync_tags

> models::SyncTagsResponse sync_tags(cursor)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**cursor** | Option<**i64**> | Cursor returned by the previous sync. Absent means a full sync. |  |

### Return type

[**models::SyncTagsResponse**](SyncTagsResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

