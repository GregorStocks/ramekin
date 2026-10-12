# ramekin_client.IngredientNamesApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**get_ingredient_names_status**](IngredientNamesApi.md#get_ingredient_names_status) | **GET** /api/ingredient-names/status | 
[**retry_ingredient_names**](IngredientNamesApi.md#retry_ingredient_names) | **POST** /api/ingredient-names/retry | 


# **get_ingredient_names_status**
> IngredientNamesStatusResponse get_ingredient_names_status()

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.ingredient_names_status_response import IngredientNamesStatusResponse
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
    api_instance = ramekin_client.IngredientNamesApi(api_client)

    try:
        api_response = api_instance.get_ingredient_names_status()
        print("The response of IngredientNamesApi->get_ingredient_names_status:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling IngredientNamesApi->get_ingredient_names_status: %s\n" % e)
```



### Parameters

This endpoint does not need any parameter.

### Return type

[**IngredientNamesStatusResponse**](IngredientNamesStatusResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Resolution status of ingredient names the catalog doesn&#39;t know |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **retry_ingredient_names**
> IngredientNamesQueuedResponse retry_ingredient_names()

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.ingredient_names_queued_response import IngredientNamesQueuedResponse
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
    api_instance = ramekin_client.IngredientNamesApi(api_client)

    try:
        api_response = api_instance.retry_ingredient_names()
        print("The response of IngredientNamesApi->retry_ingredient_names:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling IngredientNamesApi->retry_ingredient_names: %s\n" % e)
```



### Parameters

This endpoint does not need any parameter.

### Return type

[**IngredientNamesQueuedResponse**](IngredientNamesQueuedResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Failed names queued again |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

