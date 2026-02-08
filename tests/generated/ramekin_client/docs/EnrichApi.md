# ramekin_client.EnrichApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**custom_enrich_recipe**](EnrichApi.md#custom_enrich_recipe) | **POST** /api/enrich/custom | Apply a custom AI modification to a recipe
[**enrich_recipe**](EnrichApi.md#enrich_recipe) | **POST** /api/enrich | Enrich a recipe


# **custom_enrich_recipe**
> RecipeContent custom_enrich_recipe(custom_enrich_request)

Apply a custom AI modification to a recipe

Takes a recipe and a free-text instruction describing the desired change.
Returns the complete modified recipe. Stateless - does NOT modify any database records.

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.custom_enrich_request import CustomEnrichRequest
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
    custom_enrich_request = ramekin_client.CustomEnrichRequest() # CustomEnrichRequest | 

    try:
        # Apply a custom AI modification to a recipe
        api_response = api_instance.custom_enrich_recipe(custom_enrich_request)
        print("The response of EnrichApi->custom_enrich_recipe:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling EnrichApi->custom_enrich_recipe: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **custom_enrich_request** | [**CustomEnrichRequest**](CustomEnrichRequest.md)|  | 

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
**200** | Modified recipe |  -  |
**401** | Unauthorized |  -  |
**503** | AI service unavailable |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **enrich_recipe**
> RecipeContent enrich_recipe(recipe_content)

Enrich a recipe

This is a stateless endpoint that takes a recipe object and returns an enriched version.
It does NOT modify any database records. The client can apply the enriched data
via a normal PUT /api/recipes/{id} call.

Enriches:
- Ingredient measurements with gram conversions (volume/weight → grams)
- Tags by suggesting from the user's existing tag library (requires AI; skipped if unavailable)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
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
    recipe_content = ramekin_client.RecipeContent() # RecipeContent | 

    try:
        # Enrich a recipe
        api_response = api_instance.enrich_recipe(recipe_content)
        print("The response of EnrichApi->enrich_recipe:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling EnrichApi->enrich_recipe: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **recipe_content** | [**RecipeContent**](RecipeContent.md)|  | 

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
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

