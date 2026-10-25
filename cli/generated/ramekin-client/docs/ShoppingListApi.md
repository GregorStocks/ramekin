# \ShoppingListApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**list_shopping_list_history**](ShoppingListApi.md#list_shopping_list_history) | **GET** /api/shopping-list/history | 
[**undo_shopping_list_change**](ShoppingListApi.md#undo_shopping_list_change) | **POST** /api/shopping-list/history/{id}/undo | 



## list_shopping_list_history

> models::ShoppingListHistoryResponse list_shopping_list_history(limit)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**limit** | Option<**i64**> | Most entries to return, between 1 and 200. Defaults to 50. |  |

### Return type

[**models::ShoppingListHistoryResponse**](ShoppingListHistoryResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## undo_shopping_list_change

> models::ShoppingListHistoryEntry undo_shopping_list_change(id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | History entry ID | [required] |

### Return type

[**models::ShoppingListHistoryEntry**](ShoppingListHistoryEntry.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

