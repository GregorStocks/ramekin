# ramekin_client.DefaultApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**get_garbages**](DefaultApi.md#get_garbages) | **GET** /api/garbages | 


# **get_garbages**
> GarbagesResponse get_garbages()



### Example


```python
import ramekin_client
from ramekin_client.models.garbages_response import GarbagesResponse
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
        api_response = api_instance.get_garbages()
        print("The response of DefaultApi->get_garbages:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling DefaultApi->get_garbages: %s\n" % e)
```



### Parameters

This endpoint does not need any parameter.

### Return type

[**GarbagesResponse**](GarbagesResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | List of all garbages |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

