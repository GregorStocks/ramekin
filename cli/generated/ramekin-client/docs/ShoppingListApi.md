# \ShoppingListApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**clear_checked**](ShoppingListApi.md#clear_checked) | **DELETE** /api/shopping-list/clear-checked | 
[**create_items**](ShoppingListApi.md#create_items) | **POST** /api/shopping-list | 
[**delete_item**](ShoppingListApi.md#delete_item) | **DELETE** /api/shopping-list/{id} | 
[**list_items**](ShoppingListApi.md#list_items) | **GET** /api/shopping-list | 
[**sync_items**](ShoppingListApi.md#sync_items) | **POST** /api/shopping-list/sync | 
[**update_item**](ShoppingListApi.md#update_item) | **PUT** /api/shopping-list/{id} | 



## clear_checked

> models::ClearCheckedResponse clear_checked()


### Parameters

This endpoint does not need any parameter.

### Return type

[**models::ClearCheckedResponse**](ClearCheckedResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## create_items

> models::CreateShoppingListResponse create_items(create_shopping_list_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**create_shopping_list_request** | [**CreateShoppingListRequest**](CreateShoppingListRequest.md) |  | [required] |

### Return type

[**models::CreateShoppingListResponse**](CreateShoppingListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## delete_item

> delete_item(id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Shopping list item ID | [required] |

### Return type

 (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## list_items

> models::ShoppingListResponse list_items()


### Parameters

This endpoint does not need any parameter.

### Return type

[**models::ShoppingListResponse**](ShoppingListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## sync_items

> models::SyncResponse sync_items(sync_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**sync_request** | [**SyncRequest**](SyncRequest.md) |  | [required] |

### Return type

[**models::SyncResponse**](SyncResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## update_item

> update_item(id, update_shopping_list_item_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Shopping list item ID | [required] |
**update_shopping_list_item_request** | [**UpdateShoppingListItemRequest**](UpdateShoppingListItemRequest.md) |  | [required] |

### Return type

 (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

