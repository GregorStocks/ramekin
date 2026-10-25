# ramekin_client.EditRecipeApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**edit_recipe_with_ai**](EditRecipeApi.md#edit_recipe_with_ai) | **POST** /api/recipes/{id}/edit-with-ai | Edit a recipe with AI


# **edit_recipe_with_ai**
> NormalizeJobResponse edit_recipe_with_ai(id, edit_recipe_request)

Edit a recipe with AI

Starts a job that edits the recipe's current version the way the
instruction says, looking up catalog weights and calories and the user's
tags and recipes as it goes. Poll it with `GET /api/scrape/{id}`; once it
completes, `GET /api/recipes/{id}/normalize/{job_id}` returns the proposed
version and a summary of the changes, and
`POST /api/recipes/{id}/normalize/{job_id}/accept` saves it.

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.edit_recipe_request import EditRecipeRequest
from ramekin_client.models.normalize_job_response import NormalizeJobResponse
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
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Recipe ID
    edit_recipe_request = ramekin_client.EditRecipeRequest() # EditRecipeRequest | 

    try:
        # Edit a recipe with AI
        api_response = api_instance.edit_recipe_with_ai(id, edit_recipe_request)
        print("The response of EditRecipeApi->edit_recipe_with_ai:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling EditRecipeApi->edit_recipe_with_ai: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Recipe ID | 
 **edit_recipe_request** | [**EditRecipeRequest**](EditRecipeRequest.md)|  | 

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
**201** | Edit job started; it proposes a version |  -  |
**400** | Empty instruction, or invalid photo IDs |  -  |
**401** | Unauthorized |  -  |
**404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

