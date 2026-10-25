# ramekin_client.NormalizeApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**accept_normalize_proposal**](NormalizeApi.md#accept_normalize_proposal) | **POST** /api/recipes/{id}/normalize/{job_id}/accept | 
[**get_normalize_proposal**](NormalizeApi.md#get_normalize_proposal) | **GET** /api/recipes/{id}/normalize/{job_id} | 
[**list_normalize_ops**](NormalizeApi.md#list_normalize_ops) | **GET** /api/recipes/normalize-ops | 
[**normalize_recipe**](NormalizeApi.md#normalize_recipe) | **POST** /api/recipes/{id}/normalize | 
[**normalize_recipes**](NormalizeApi.md#normalize_recipes) | **POST** /api/recipes/normalize | 


# **accept_normalize_proposal**
> AcceptNormalizeResponse accept_normalize_proposal(id, job_id)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.accept_normalize_response import AcceptNormalizeResponse
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
    api_instance = ramekin_client.NormalizeApi(api_client)
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Recipe ID
    job_id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Normalize job ID

    try:
        api_response = api_instance.accept_normalize_proposal(id, job_id)
        print("The response of NormalizeApi->accept_normalize_proposal:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling NormalizeApi->accept_normalize_proposal: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Recipe ID | 
 **job_id** | **UUID**| Normalize job ID | 

### Return type

[**AcceptNormalizeResponse**](AcceptNormalizeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**201** | Proposal saved as the recipe&#39;s new version |  -  |
**400** | The job has not finished proposing a version |  -  |
**401** | Unauthorized |  -  |
**404** | Normalize job not found |  -  |
**409** | The recipe changed since the job started |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **get_normalize_proposal**
> NormalizeProposalResponse get_normalize_proposal(id, job_id)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.normalize_proposal_response import NormalizeProposalResponse
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
    api_instance = ramekin_client.NormalizeApi(api_client)
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Recipe ID
    job_id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Normalize job ID

    try:
        api_response = api_instance.get_normalize_proposal(id, job_id)
        print("The response of NormalizeApi->get_normalize_proposal:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling NormalizeApi->get_normalize_proposal: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Recipe ID | 
 **job_id** | **UUID**| Normalize job ID | 

### Return type

[**NormalizeProposalResponse**](NormalizeProposalResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | The job&#39;s base version and, once it completes, its proposal |  -  |
**401** | Unauthorized |  -  |
**404** | Normalize job not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **list_normalize_ops**
> NormalizeOpsResponse list_normalize_ops(recipe_id=recipe_id)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.normalize_ops_response import NormalizeOpsResponse
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
    api_instance = ramekin_client.NormalizeApi(api_client)
    recipe_id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Report which ops can run on this recipe. (optional)

    try:
        api_response = api_instance.list_normalize_ops(recipe_id=recipe_id)
        print("The response of NormalizeApi->list_normalize_ops:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling NormalizeApi->list_normalize_ops: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **recipe_id** | **UUID**| Report which ops can run on this recipe. | [optional] 

### Return type

[**NormalizeOpsResponse**](NormalizeOpsResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Every normalize op, in run order |  -  |
**401** | Unauthorized |  -  |
**404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **normalize_recipe**
> NormalizeJobResponse normalize_recipe(id, normalize_request)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.normalize_job_response import NormalizeJobResponse
from ramekin_client.models.normalize_request import NormalizeRequest
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
    api_instance = ramekin_client.NormalizeApi(api_client)
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Recipe ID
    normalize_request = ramekin_client.NormalizeRequest() # NormalizeRequest | 

    try:
        api_response = api_instance.normalize_recipe(id, normalize_request)
        print("The response of NormalizeApi->normalize_recipe:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling NormalizeApi->normalize_recipe: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Recipe ID | 
 **normalize_request** | [**NormalizeRequest**](NormalizeRequest.md)|  | 

### Return type

[**NormalizeJobResponse**](NormalizeJobResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**201** | Normalize job started; it proposes a version |  -  |
**400** | Unknown op, or an op unavailable for this recipe |  -  |
**401** | Unauthorized |  -  |
**404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **normalize_recipes**
> BulkNormalizeResponse normalize_recipes(bulk_normalize_request)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.bulk_normalize_request import BulkNormalizeRequest
from ramekin_client.models.bulk_normalize_response import BulkNormalizeResponse
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
    api_instance = ramekin_client.NormalizeApi(api_client)
    bulk_normalize_request = ramekin_client.BulkNormalizeRequest() # BulkNormalizeRequest | 

    try:
        api_response = api_instance.normalize_recipes(bulk_normalize_request)
        print("The response of NormalizeApi->normalize_recipes:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling NormalizeApi->normalize_recipes: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **bulk_normalize_request** | [**BulkNormalizeRequest**](BulkNormalizeRequest.md)|  | 

### Return type

[**BulkNormalizeResponse**](BulkNormalizeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | One result per recipe: its job, or why none started |  -  |
**400** | Unknown op |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

