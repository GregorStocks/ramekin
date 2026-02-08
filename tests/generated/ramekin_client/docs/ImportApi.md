# ramekin_client.ImportApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**import_from_photos**](ImportApi.md#import_from_photos) | **POST** /api/import/photos | 
[**import_recipe**](ImportApi.md#import_recipe) | **POST** /api/import/recipe | 


# **import_from_photos**
> ImportFromPhotosResponse import_from_photos(import_from_photos_request)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.import_from_photos_request import ImportFromPhotosRequest
from ramekin_client.models.import_from_photos_response import ImportFromPhotosResponse
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
    api_instance = ramekin_client.ImportApi(api_client)
    import_from_photos_request = ramekin_client.ImportFromPhotosRequest() # ImportFromPhotosRequest | 

    try:
        api_response = api_instance.import_from_photos(import_from_photos_request)
        print("The response of ImportApi->import_from_photos:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ImportApi->import_from_photos: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **import_from_photos_request** | [**ImportFromPhotosRequest**](ImportFromPhotosRequest.md)|  | 

### Return type

[**ImportFromPhotosResponse**](ImportFromPhotosResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**201** | Photo import job created |  -  |
**400** | Invalid request |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **import_recipe**
> ImportRecipeResponse import_recipe(import_recipe_request)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.import_recipe_request import ImportRecipeRequest
from ramekin_client.models.import_recipe_response import ImportRecipeResponse
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
    api_instance = ramekin_client.ImportApi(api_client)
    import_recipe_request = ramekin_client.ImportRecipeRequest() # ImportRecipeRequest | 

    try:
        api_response = api_instance.import_recipe(import_recipe_request)
        print("The response of ImportApi->import_recipe:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ImportApi->import_recipe: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **import_recipe_request** | [**ImportRecipeRequest**](ImportRecipeRequest.md)|  | 

### Return type

[**ImportRecipeResponse**](ImportRecipeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**201** | Import job created |  -  |
**400** | Invalid request |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

