# ramekin_client.UsersApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**me**](UsersApi.md#me) | **GET** /api/users/me | 
[**mint_bookmarklet_token**](UsersApi.md#mint_bookmarklet_token) | **POST** /api/users/bookmarklet-token | 


# **me**
> MeResponse me()

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.me_response import MeResponse
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
    api_instance = ramekin_client.UsersApi(api_client)

    try:
        api_response = api_instance.me()
        print("The response of UsersApi->me:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling UsersApi->me: %s\n" % e)
```



### Parameters

This endpoint does not need any parameter.

### Return type

[**MeResponse**](MeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | The currently authenticated user |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **mint_bookmarklet_token**
> BookmarkletTokenResponse mint_bookmarklet_token()

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.bookmarklet_token_response import BookmarkletTokenResponse
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
    api_instance = ramekin_client.UsersApi(api_client)

    try:
        api_response = api_instance.mint_bookmarklet_token()
        print("The response of UsersApi->mint_bookmarklet_token:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling UsersApi->mint_bookmarklet_token: %s\n" % e)
```



### Parameters

This endpoint does not need any parameter.

### Return type

[**BookmarkletTokenResponse**](BookmarkletTokenResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**201** | A freshly minted bookmarklet token |  -  |
**401** | Unauthorized |  -  |
**403** | A bookmarklet token may not mint tokens |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

