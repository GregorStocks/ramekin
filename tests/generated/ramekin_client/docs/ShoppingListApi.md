# ramekin_client.ShoppingListApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**list_shopping_list_history**](ShoppingListApi.md#list_shopping_list_history) | **GET** /api/shopping-list/history | 
[**undo_shopping_list_change**](ShoppingListApi.md#undo_shopping_list_change) | **POST** /api/shopping-list/history/{id}/undo | 


# **list_shopping_list_history**
> ShoppingListHistoryResponse list_shopping_list_history(limit=limit)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.shopping_list_history_response import ShoppingListHistoryResponse
from ramekin_client.rest import ApiException
from pprint import pprint

# Defining the host is optional and defaults to http://localhost
# See configuration.py for a list of all supported configuration parameters.
configuration = ramekin_client.Configuration(
    host = "http://localhost"
)

# The client must configure the authentication and authorization parameters
# in accordance with the API server security policy.
# Examples for each auth method are provided below, use the example that
# satisfies your auth use case.

# Configure Bearer authorization: bearer_auth
configuration = ramekin_client.Configuration(
    access_token = os.environ["BEARER_TOKEN"]
)

# Enter a context with an instance of the API client
with ramekin_client.ApiClient(configuration) as api_client:
    # Create an instance of the API class
    api_instance = ramekin_client.ShoppingListApi(api_client)
    limit = 56 # int | Most entries to return, between 1 and 200. Defaults to 50. (optional)

    try:
        api_response = api_instance.list_shopping_list_history(limit=limit)
        print("The response of ShoppingListApi->list_shopping_list_history:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ShoppingListApi->list_shopping_list_history: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **limit** | **int**| Most entries to return, between 1 and 200. Defaults to 50. | [optional] 

### Return type

[**ShoppingListHistoryResponse**](ShoppingListHistoryResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | The latest history entries |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **undo_shopping_list_change**
> ShoppingListHistoryEntry undo_shopping_list_change(id)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.shopping_list_history_entry import ShoppingListHistoryEntry
from ramekin_client.rest import ApiException
from pprint import pprint

# Defining the host is optional and defaults to http://localhost
# See configuration.py for a list of all supported configuration parameters.
configuration = ramekin_client.Configuration(
    host = "http://localhost"
)

# The client must configure the authentication and authorization parameters
# in accordance with the API server security policy.
# Examples for each auth method are provided below, use the example that
# satisfies your auth use case.

# Configure Bearer authorization: bearer_auth
configuration = ramekin_client.Configuration(
    access_token = os.environ["BEARER_TOKEN"]
)

# Enter a context with an instance of the API client
with ramekin_client.ApiClient(configuration) as api_client:
    # Create an instance of the API class
    api_instance = ramekin_client.ShoppingListApi(api_client)
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | History entry ID

    try:
        api_response = api_instance.undo_shopping_list_change(id)
        print("The response of ShoppingListApi->undo_shopping_list_change:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ShoppingListApi->undo_shopping_list_change: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| History entry ID | 

### Return type

[**ShoppingListHistoryEntry**](ShoppingListHistoryEntry.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Undone; the entry recording the undo. Sync to see it. |  -  |
**401** | Unauthorized |  -  |
**404** | No such entry |  -  |
**409** | Already undone, or later changes replaced all of it |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

