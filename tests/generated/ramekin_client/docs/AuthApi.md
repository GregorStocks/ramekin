# ramekin_client.AuthApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**hello**](AuthApi.md#hello) | **GET** /api/auth/hello | 
[**login**](AuthApi.md#login) | **POST** /api/auth/login | 
[**signup**](AuthApi.md#signup) | **POST** /api/auth/signup | 


# **hello**
> HelloResponse hello()



### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.hello_response import HelloResponse
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
    api_instance = ramekin_client.AuthApi(api_client)

    try:
        api_response = api_instance.hello()
        print("The response of AuthApi->hello:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling AuthApi->hello: %s\n" % e)
```



### Parameters

This endpoint does not need any parameter.

### Return type

[**HelloResponse**](HelloResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Hello message |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **login**
> LoginResponse login(login_request)



### Example


```python
import ramekin_client
from ramekin_client.models.login_request import LoginRequest
from ramekin_client.models.login_response import LoginResponse
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
    api_instance = ramekin_client.AuthApi(api_client)
    login_request = ramekin_client.LoginRequest() # LoginRequest | 

    try:
        api_response = api_instance.login(login_request)
        print("The response of AuthApi->login:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling AuthApi->login: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **login_request** | [**LoginRequest**](LoginRequest.md)|  | 

### Return type

[**LoginResponse**](LoginResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Login successful |  -  |
**401** | Invalid credentials |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **signup**
> SignupResponse signup(signup_request)



### Example


```python
import ramekin_client
from ramekin_client.models.signup_request import SignupRequest
from ramekin_client.models.signup_response import SignupResponse
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
    api_instance = ramekin_client.AuthApi(api_client)
    signup_request = ramekin_client.SignupRequest() # SignupRequest | 

    try:
        api_response = api_instance.signup(signup_request)
        print("The response of AuthApi->signup:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling AuthApi->signup: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **signup_request** | [**SignupRequest**](SignupRequest.md)|  | 

### Return type

[**SignupResponse**](SignupResponse.md)

### Authorization

No authorization required

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**201** | User created successfully |  -  |
**400** | Invalid request |  -  |
**409** | Username already exists |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

