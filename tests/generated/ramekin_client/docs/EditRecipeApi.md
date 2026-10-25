# ramekin_client.EditRecipeApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**edit_recipe_with_ai**](EditRecipeApi.md#edit_recipe_with_ai) | **POST** /api/edit-recipe | Edit a recipe with AI


# **edit_recipe_with_ai**
> RecipeContent edit_recipe_with_ai(edit_recipe_request)

Edit a recipe with AI

Takes a recipe and a free-text instruction describing the desired change.
Returns the complete modified recipe. Stateless - does NOT modify any database records.

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.edit_recipe_request import EditRecipeRequest
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
    api_instance = ramekin_client.EditRecipeApi(api_client)
    edit_recipe_request = ramekin_client.EditRecipeRequest() # EditRecipeRequest | 

    try:
        # Edit a recipe with AI
        api_response = api_instance.edit_recipe_with_ai(edit_recipe_request)
        print("The response of EditRecipeApi->edit_recipe_with_ai:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling EditRecipeApi->edit_recipe_with_ai: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **edit_recipe_request** | [**EditRecipeRequest**](EditRecipeRequest.md)|  | 

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
**400** | Invalid photo IDs |  -  |
**401** | Unauthorized |  -  |
**500** | Internal server error |  -  |
**503** | AI service unavailable |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

