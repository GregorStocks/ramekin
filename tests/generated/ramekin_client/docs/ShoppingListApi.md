# ramekin_client.ShoppingListApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**clear_checked**](ShoppingListApi.md#clear_checked) | **DELETE** /api/shopping-list/clear-checked | 
[**create_items**](ShoppingListApi.md#create_items) | **POST** /api/shopping-list | 
[**delete_item**](ShoppingListApi.md#delete_item) | **DELETE** /api/shopping-list/{id} | 
[**list_items**](ShoppingListApi.md#list_items) | **GET** /api/shopping-list | 
[**sync_items**](ShoppingListApi.md#sync_items) | **POST** /api/shopping-list/sync | 
[**update_item**](ShoppingListApi.md#update_item) | **PUT** /api/shopping-list/{id} | 


# **clear_checked**
> ClearCheckedResponse clear_checked()

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.clear_checked_response import ClearCheckedResponse
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

    try:
        api_response = api_instance.clear_checked()
        print("The response of ShoppingListApi->clear_checked:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ShoppingListApi->clear_checked: %s\n" % e)
```



### Parameters

This endpoint does not need any parameter.

### Return type

[**ClearCheckedResponse**](ClearCheckedResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Checked items cleared |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **create_items**
> CreateShoppingListResponse create_items(create_shopping_list_request)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.create_shopping_list_request import CreateShoppingListRequest
from ramekin_client.models.create_shopping_list_response import CreateShoppingListResponse
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
    create_shopping_list_request = ramekin_client.CreateShoppingListRequest() # CreateShoppingListRequest | 

    try:
        api_response = api_instance.create_items(create_shopping_list_request)
        print("The response of ShoppingListApi->create_items:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ShoppingListApi->create_items: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **create_shopping_list_request** | [**CreateShoppingListRequest**](CreateShoppingListRequest.md)|  | 

### Return type

[**CreateShoppingListResponse**](CreateShoppingListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**201** | Items created |  -  |
**400** | Invalid request |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **delete_item**
> delete_item(id)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
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
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Shopping list item ID

    try:
        api_instance.delete_item(id)
    except Exception as e:
        print("Exception when calling ShoppingListApi->delete_item: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Shopping list item ID | 

### Return type

void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**204** | Item deleted |  -  |
**401** | Unauthorized |  -  |
**404** | Item not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **list_items**
> ShoppingListResponse list_items()

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.shopping_list_response import ShoppingListResponse
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

    try:
        api_response = api_instance.list_items()
        print("The response of ShoppingListApi->list_items:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ShoppingListApi->list_items: %s\n" % e)
```



### Parameters

This endpoint does not need any parameter.

### Return type

[**ShoppingListResponse**](ShoppingListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | List of shopping list items |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **sync_items**
> SyncResponse sync_items(sync_request)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.sync_request import SyncRequest
from ramekin_client.models.sync_response import SyncResponse
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
    sync_request = ramekin_client.SyncRequest() # SyncRequest | 

    try:
        api_response = api_instance.sync_items(sync_request)
        print("The response of ShoppingListApi->sync_items:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ShoppingListApi->sync_items: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **sync_request** | [**SyncRequest**](SyncRequest.md)|  | 

### Return type

[**SyncResponse**](SyncResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Sync completed |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **update_item**
> update_item(id, update_shopping_list_item_request)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.update_shopping_list_item_request import UpdateShoppingListItemRequest
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
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Shopping list item ID
    update_shopping_list_item_request = ramekin_client.UpdateShoppingListItemRequest() # UpdateShoppingListItemRequest | 

    try:
        api_instance.update_item(id, update_shopping_list_item_request)
    except Exception as e:
        print("Exception when calling ShoppingListApi->update_item: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Shopping list item ID | 
 **update_shopping_list_item_request** | [**UpdateShoppingListItemRequest**](UpdateShoppingListItemRequest.md)|  | 

### Return type

void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Item updated |  -  |
**401** | Unauthorized |  -  |
**404** | Item not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

