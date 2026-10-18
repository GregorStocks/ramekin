# ramekin_client.SyncApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**sync_meal_plans**](SyncApi.md#sync_meal_plans) | **POST** /api/sync/meal-plans | 
[**sync_recipes**](SyncApi.md#sync_recipes) | **GET** /api/sync/recipes | 
[**sync_shopping_list**](SyncApi.md#sync_shopping_list) | **POST** /api/sync/shopping-list | 
[**sync_tags**](SyncApi.md#sync_tags) | **GET** /api/sync/tags | 


# **sync_meal_plans**
> SyncMealPlansResponse sync_meal_plans(sync_meal_plans_request)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.sync_meal_plans_request import SyncMealPlansRequest
from ramekin_client.models.sync_meal_plans_response import SyncMealPlansResponse
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
    api_instance = ramekin_client.SyncApi(api_client)
    sync_meal_plans_request = ramekin_client.SyncMealPlansRequest() # SyncMealPlansRequest | 

    try:
        api_response = api_instance.sync_meal_plans(sync_meal_plans_request)
        print("The response of SyncApi->sync_meal_plans:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling SyncApi->sync_meal_plans: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **sync_meal_plans_request** | [**SyncMealPlansRequest**](SyncMealPlansRequest.md)|  | 

### Return type

[**SyncMealPlansResponse**](SyncMealPlansResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Ops applied; changes since the cursor |  -  |
**400** | A malformed op |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **sync_recipes**
> SyncRecipesResponse sync_recipes(limit, cursor=cursor, after_id=after_id)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.sync_recipes_response import SyncRecipesResponse
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
    api_instance = ramekin_client.SyncApi(api_client)
    limit = 56 # int | Maximum number of recipes returned per page, between 1 and 500.
    cursor = 56 # int | Cursor returned by a previous sync. Absent means a full sync. Every page of one sweep passes the same cursor. (optional)
    after_id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | ID of the last recipe on the previous page. Absent on a sweep's first page. (optional)

    try:
        api_response = api_instance.sync_recipes(limit, cursor=cursor, after_id=after_id)
        print("The response of SyncApi->sync_recipes:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling SyncApi->sync_recipes: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **limit** | **int**| Maximum number of recipes returned per page, between 1 and 500. | 
 **cursor** | **int**| Cursor returned by a previous sync. Absent means a full sync. Every page of one sweep passes the same cursor. | [optional] 
 **after_id** | **UUID**| ID of the last recipe on the previous page. Absent on a sweep&#39;s first page. | [optional] 

### Return type

[**SyncRecipesResponse**](SyncRecipesResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Recipe changes for local cache sync |  -  |
**400** | Invalid limit |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **sync_shopping_list**
> SyncShoppingListResponse sync_shopping_list(sync_shopping_list_request)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.sync_shopping_list_request import SyncShoppingListRequest
from ramekin_client.models.sync_shopping_list_response import SyncShoppingListResponse
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
    api_instance = ramekin_client.SyncApi(api_client)
    sync_shopping_list_request = ramekin_client.SyncShoppingListRequest() # SyncShoppingListRequest | 

    try:
        api_response = api_instance.sync_shopping_list(sync_shopping_list_request)
        print("The response of SyncApi->sync_shopping_list:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling SyncApi->sync_shopping_list: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **sync_shopping_list_request** | [**SyncShoppingListRequest**](SyncShoppingListRequest.md)|  | 

### Return type

[**SyncShoppingListResponse**](SyncShoppingListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Ops applied; changes since the cursor |  -  |
**400** | A malformed op |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **sync_tags**
> SyncTagsResponse sync_tags(cursor=cursor)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.sync_tags_response import SyncTagsResponse
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
    api_instance = ramekin_client.SyncApi(api_client)
    cursor = 56 # int | Cursor returned by the previous sync. Absent means a full sync. (optional)

    try:
        api_response = api_instance.sync_tags(cursor=cursor)
        print("The response of SyncApi->sync_tags:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling SyncApi->sync_tags: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **cursor** | **int**| Cursor returned by the previous sync. Absent means a full sync. | [optional] 

### Return type

[**SyncTagsResponse**](SyncTagsResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Tag changes for the local cache |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

