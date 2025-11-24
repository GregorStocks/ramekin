# ramekin_client.DefaultApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**unauthed_ping**](DefaultApi.md#unauthed_ping) | **GET** /api/test/unauthed-ping | 


# **unauthed_ping**
> PingResponse unauthed_ping()



### Example


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


# Enter a context with an instance of the API client
with ramekin_client.ApiClient(configuration) as api_client:
    # Create an instance of the API class
    api_instance = ramekin_client.DefaultApi(api_client)

    try:
        api_response = api_instance.unauthed_ping()
        print("The response of DefaultApi->unauthed_ping:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling DefaultApi->unauthed_ping: %s\n" % e)
```



### Parameters

This endpoint does not need any parameter.

### Return type

[**PingResponse**](PingResponse.md)

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

