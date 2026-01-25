# ramekin_client.EnrichApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**enrich_recipe**](EnrichApi.md#enrich_recipe) | **POST** /api/enrich | Enrich a recipe using AI
[**list_enrichments**](EnrichApi.md#list_enrichments) | **GET** /api/enrichments | List available enrichment types


# **enrich_recipe**
> RecipeContent enrich_recipe(enrich_request)

Enrich a recipe using AI

This is a stateless endpoint that takes a recipe object and returns an enriched version.
It does NOT modify any database records. The client can apply the enriched data
via a normal PUT /api/recipes/{id} call.

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.enrich_request import EnrichRequest
from ramekin_client.models.recipe_content import RecipeContent
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
    api_instance = ramekin_client.EnrichApi(api_client)
    enrich_request = ramekin_client.EnrichRequest() # EnrichRequest | 

    try:
        # Enrich a recipe using AI
        api_response = api_instance.enrich_recipe(enrich_request)
        print("The response of EnrichApi->enrich_recipe:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling EnrichApi->enrich_recipe: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **enrich_request** | [**EnrichRequest**](EnrichRequest.md)|  | 

### Return type

[**RecipeContent**](RecipeContent.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Enriched recipe object |  -  |
**400** | Invalid enrichment type |  -  |
**401** | Unauthorized |  -  |
**503** | AI service unavailable |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **list_enrichments**
> ListEnrichmentsResponse list_enrichments()

List available enrichment types

Returns information about all available enrichment types, including their
names, descriptions, and which recipe fields they modify.

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.list_enrichments_response import ListEnrichmentsResponse
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
    api_instance = ramekin_client.EnrichApi(api_client)

    try:
        # List available enrichment types
        api_response = api_instance.list_enrichments()
        print("The response of EnrichApi->list_enrichments:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling EnrichApi->list_enrichments: %s\n" % e)
```



### Parameters

This endpoint does not need any parameter.

### Return type

[**ListEnrichmentsResponse**](ListEnrichmentsResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | List of available enrichments |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

