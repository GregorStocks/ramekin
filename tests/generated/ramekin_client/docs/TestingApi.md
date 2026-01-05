# ramekin_client.TestingApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**ping**](TestingApi.md#ping) | **GET** /api/test/ping | 
[**unauthed_ping**](TestingApi.md#unauthed_ping) | **GET** /api/test/unauthed-ping | 


# **ping**
> PingResponse ping()

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.ping_response import PingResponse
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
    api_instance = ramekin_client.TestingApi(api_client)

    try:
        api_response = api_instance.ping()
        print("The response of TestingApi->ping:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling TestingApi->ping: %s\n" % e)
```



### Parameters

This endpoint does not need any parameter.

### Return type

[**PingResponse**](PingResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Authenticated ping response |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **unauthed_ping**
> UnauthedPingResponse unauthed_ping()

### Example


```python
import ramekin_client
from ramekin_client.models.unauthed_ping_response import UnauthedPingResponse
from ramekin_client.rest import ApiException
from pprint import pprint

# Defining the host is optional and defaults to http://localhost
# See configuration.py for a list of all supported configuration parameters.
configuration = ramekin_client.Configuration(
    host = "http://localhost"
)


# Enter a context with an instance of the API client
with ramekin_client.ApiClient(configuration) as api_client:
    # Create an instance of the API class
    api_instance = ramekin_client.TestingApi(api_client)

    try:
        api_response = api_instance.unauthed_ping()
        print("The response of TestingApi->unauthed_ping:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling TestingApi->unauthed_ping: %s\n" % e)
```



### Parameters

This endpoint does not need any parameter.

### Return type

[**UnauthedPingResponse**](UnauthedPingResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Unauthed ping response |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

