# ramekin_client.ClientLogsApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_client_log**](ClientLogsApi.md#create_client_log) | **POST** /api/client-logs | 


# **create_client_log**
> CreateClientLogResponse create_client_log(create_client_log_request)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.create_client_log_request import CreateClientLogRequest
from ramekin_client.models.create_client_log_response import CreateClientLogResponse
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
    api_instance = ramekin_client.ClientLogsApi(api_client)
    create_client_log_request = ramekin_client.CreateClientLogRequest() # CreateClientLogRequest | 

    try:
        api_response = api_instance.create_client_log(create_client_log_request)
        print("The response of ClientLogsApi->create_client_log:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling ClientLogsApi->create_client_log: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **create_client_log_request** | [**CreateClientLogRequest**](CreateClientLogRequest.md)|  | 

### Return type

[**CreateClientLogResponse**](CreateClientLogResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**201** | Log upload stored |  -  |
**400** | Invalid request |  -  |
**401** | Unauthorized |  -  |
**413** | Content too large |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

