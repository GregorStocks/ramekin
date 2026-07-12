# ramekin_client.RecipesApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_recipe**](RecipesApi.md#create_recipe) | **POST** /api/recipes | 
[**delete_recipe**](RecipesApi.md#delete_recipe) | **DELETE** /api/recipes/{id} | 
[**export_all_recipes**](RecipesApi.md#export_all_recipes) | **GET** /api/recipes/export | 
[**export_recipe**](RecipesApi.md#export_recipe) | **GET** /api/recipes/{id}/export | 
[**generate_description**](RecipesApi.md#generate_description) | **POST** /api/recipes/{id}/generate-description | 
[**generate_photo**](RecipesApi.md#generate_photo) | **POST** /api/recipes/{id}/generate-photo | 
[**get_recipe**](RecipesApi.md#get_recipe) | **GET** /api/recipes/{id} | 
[**list_recipes**](RecipesApi.md#list_recipes) | **GET** /api/recipes | 
[**list_versions**](RecipesApi.md#list_versions) | **GET** /api/recipes/{id}/versions | 
[**normalize_title**](RecipesApi.md#normalize_title) | **POST** /api/recipes/{id}/normalize-title | 
[**rescrape**](RecipesApi.md#rescrape) | **POST** /api/recipes/{id}/rescrape | 
[**rescrape_photo**](RecipesApi.md#rescrape_photo) | **POST** /api/recipes/{id}/rescrape-photo | 
[**sync_recipes**](RecipesApi.md#sync_recipes) | **GET** /api/recipes/sync | 
[**update_recipe**](RecipesApi.md#update_recipe) | **PUT** /api/recipes/{id} | 


# **create_recipe**
> CreateRecipeResponse create_recipe(create_recipe_request)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.create_recipe_request import CreateRecipeRequest
from ramekin_client.models.create_recipe_response import CreateRecipeResponse
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
    api_instance = ramekin_client.RecipesApi(api_client)
    create_recipe_request = ramekin_client.CreateRecipeRequest() # CreateRecipeRequest | 

    try:
        api_response = api_instance.create_recipe(create_recipe_request)
        print("The response of RecipesApi->create_recipe:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling RecipesApi->create_recipe: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **create_recipe_request** | [**CreateRecipeRequest**](CreateRecipeRequest.md)|  | 

### Return type

[**CreateRecipeResponse**](CreateRecipeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**201** | Recipe created successfully |  -  |
**400** | Invalid request |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **delete_recipe**
> delete_recipe(id)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
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
    api_instance = ramekin_client.RecipesApi(api_client)
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Recipe ID

    try:
        api_instance.delete_recipe(id)
    except Exception as e:
        print("Exception when calling RecipesApi->delete_recipe: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Recipe ID | 

### Return type

void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**204** | Recipe deleted successfully |  -  |
**401** | Unauthorized |  -  |
**404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **export_all_recipes**
> export_all_recipes()

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
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
    api_instance = ramekin_client.RecipesApi(api_client)

    try:
        api_instance.export_all_recipes()
    except Exception as e:
        print("Exception when calling RecipesApi->export_all_recipes: %s\n" % e)
```



### Parameters

This endpoint does not need any parameter.

### Return type

void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/zip, application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Paprika recipes archive (.paprikarecipes) |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **export_recipe**
> export_recipe(id)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
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
    api_instance = ramekin_client.RecipesApi(api_client)
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Recipe ID

    try:
        api_instance.export_recipe(id)
    except Exception as e:
        print("Exception when calling RecipesApi->export_recipe: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Recipe ID | 

### Return type

void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/gzip, application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Paprika recipe file (.paprikarecipe) |  -  |
**401** | Unauthorized |  -  |
**404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **generate_description**
> GenerateDescriptionResponse generate_description(id)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.generate_description_response import GenerateDescriptionResponse
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
    api_instance = ramekin_client.RecipesApi(api_client)
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Recipe ID

    try:
        api_response = api_instance.generate_description(id)
        print("The response of RecipesApi->generate_description:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling RecipesApi->generate_description: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Recipe ID | 

### Return type

[**GenerateDescriptionResponse**](GenerateDescriptionResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Description generated and applied |  -  |
**401** | Unauthorized |  -  |
**404** | Recipe not found |  -  |
**409** | Recipe was modified concurrently |  -  |
**503** | AI service unavailable |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **generate_photo**
> GeneratePhotoResponse generate_photo(id)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.generate_photo_response import GeneratePhotoResponse
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
    api_instance = ramekin_client.RecipesApi(api_client)
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Recipe ID

    try:
        api_response = api_instance.generate_photo(id)
        print("The response of RecipesApi->generate_photo:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling RecipesApi->generate_photo: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Recipe ID | 

### Return type

[**GeneratePhotoResponse**](GeneratePhotoResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Recipe photo generated and applied |  -  |
**401** | Unauthorized |  -  |
**404** | Recipe not found |  -  |
**409** | Recipe changed during generation |  -  |
**503** | AI service unavailable |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **get_recipe**
> RecipeResponse get_recipe(id, version_id=version_id)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.recipe_response import RecipeResponse
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
    api_instance = ramekin_client.RecipesApi(api_client)
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Recipe ID
    version_id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Optional version ID to fetch a specific version instead of current (optional)

    try:
        api_response = api_instance.get_recipe(id, version_id=version_id)
        print("The response of RecipesApi->get_recipe:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling RecipesApi->get_recipe: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Recipe ID | 
 **version_id** | **UUID**| Optional version ID to fetch a specific version instead of current | [optional] 

### Return type

[**RecipeResponse**](RecipeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Recipe details |  -  |
**401** | Unauthorized |  -  |
**404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **list_recipes**
> ListRecipesResponse list_recipes(limit=limit, offset=offset, q=q, sort_by=sort_by, sort_dir=sort_dir)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.direction import Direction
from ramekin_client.models.list_recipes_response import ListRecipesResponse
from ramekin_client.models.sort_by import SortBy
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
    api_instance = ramekin_client.RecipesApi(api_client)
    limit = 56 # int | Number of items to return (default: 20, max: 1000) (optional)
    offset = 56 # int | Number of items to skip (default: 0) (optional)
    q = 'q_example' # str | Search query with optional filters. Supports: - Plain text: searches title and description - tag:value: filter by tag (can use multiple) - source:value: filter by source name - has:photos / no:photos: filter by photo presence - created:>2024-01-01: created on or after date - created:<2024-12-31: created on or before date - created:2024-01-01..2024-12-31: created in date range  Date filters name inclusive UTC calendar days.  Example: \"chicken tag:dinner tag:quick has:photos\" (optional)
    sort_by = ramekin_client.SortBy() # SortBy | Sort field. Defaults to relevance when the query has text terms, otherwise updated_at. (optional)
    sort_dir = ramekin_client.Direction() # Direction | Sort direction (default: desc). Ignored when sort_by is random or relevance. (optional)

    try:
        api_response = api_instance.list_recipes(limit=limit, offset=offset, q=q, sort_by=sort_by, sort_dir=sort_dir)
        print("The response of RecipesApi->list_recipes:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling RecipesApi->list_recipes: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **limit** | **int**| Number of items to return (default: 20, max: 1000) | [optional] 
 **offset** | **int**| Number of items to skip (default: 0) | [optional] 
 **q** | **str**| Search query with optional filters. Supports: - Plain text: searches title and description - tag:value: filter by tag (can use multiple) - source:value: filter by source name - has:photos / no:photos: filter by photo presence - created:&gt;2024-01-01: created on or after date - created:&lt;2024-12-31: created on or before date - created:2024-01-01..2024-12-31: created in date range  Date filters name inclusive UTC calendar days.  Example: \&quot;chicken tag:dinner tag:quick has:photos\&quot; | [optional] 
 **sort_by** | [**SortBy**](.md)| Sort field. Defaults to relevance when the query has text terms, otherwise updated_at. | [optional] 
 **sort_dir** | [**Direction**](.md)| Sort direction (default: desc). Ignored when sort_by is random or relevance. | [optional] 

### Return type

[**ListRecipesResponse**](ListRecipesResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | List of user&#39;s recipes |  -  |
**400** | Invalid parameters |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **list_versions**
> VersionListResponse list_versions(id)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.version_list_response import VersionListResponse
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
    api_instance = ramekin_client.RecipesApi(api_client)
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Recipe ID

    try:
        api_response = api_instance.list_versions(id)
        print("The response of RecipesApi->list_versions:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling RecipesApi->list_versions: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Recipe ID | 

### Return type

[**VersionListResponse**](VersionListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | List of recipe versions |  -  |
**401** | Unauthorized |  -  |
**404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **normalize_title**
> NormalizeTitleResponse normalize_title(id)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.normalize_title_response import NormalizeTitleResponse
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
    api_instance = ramekin_client.RecipesApi(api_client)
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Recipe ID

    try:
        api_response = api_instance.normalize_title(id)
        print("The response of RecipesApi->normalize_title:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling RecipesApi->normalize_title: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Recipe ID | 

### Return type

[**NormalizeTitleResponse**](NormalizeTitleResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Title normalized and applied |  -  |
**401** | Unauthorized |  -  |
**404** | Recipe not found |  -  |
**409** | Recipe was modified concurrently |  -  |
**503** | AI service unavailable |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **rescrape**
> RescrapeResponse rescrape(id)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.rescrape_response import RescrapeResponse
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
    api_instance = ramekin_client.RecipesApi(api_client)
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Recipe ID

    try:
        api_response = api_instance.rescrape(id)
        print("The response of RecipesApi->rescrape:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling RecipesApi->rescrape: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Recipe ID | 

### Return type

[**RescrapeResponse**](RescrapeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**201** | Rescrape job created |  -  |
**400** | Recipe has no source URL |  -  |
**401** | Unauthorized |  -  |
**404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **rescrape_photo**
> RescrapeResponse rescrape_photo(id)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.rescrape_response import RescrapeResponse
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
    api_instance = ramekin_client.RecipesApi(api_client)
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Recipe ID

    try:
        api_response = api_instance.rescrape_photo(id)
        print("The response of RecipesApi->rescrape_photo:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling RecipesApi->rescrape_photo: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Recipe ID | 

### Return type

[**RescrapeResponse**](RescrapeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**201** | Photo rescrape job created |  -  |
**400** | Recipe has no source URL |  -  |
**401** | Unauthorized |  -  |
**404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **sync_recipes**
> SyncRecipesResponse sync_recipes(last_sync_at=last_sync_at)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.sync_recipes_response import SyncRecipesResponse
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
    api_instance = ramekin_client.RecipesApi(api_client)
    last_sync_at = '2013-10-20T19:20:30+01:00' # datetime | Last sync timestamp - server will return changes since this time. (optional)

    try:
        api_response = api_instance.sync_recipes(last_sync_at=last_sync_at)
        print("The response of RecipesApi->sync_recipes:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling RecipesApi->sync_recipes: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **last_sync_at** | **datetime**| Last sync timestamp - server will return changes since this time. | [optional] 

### Return type

[**SyncRecipesResponse**](SyncRecipesResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Recipe changes for local cache sync |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **update_recipe**
> update_recipe(id, update_recipe_request)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.update_recipe_request import UpdateRecipeRequest
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
    api_instance = ramekin_client.RecipesApi(api_client)
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Recipe ID
    update_recipe_request = ramekin_client.UpdateRecipeRequest() # UpdateRecipeRequest | 

    try:
        api_instance.update_recipe(id, update_recipe_request)
    except Exception as e:
        print("Exception when calling RecipesApi->update_recipe: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Recipe ID | 
 **update_recipe_request** | [**UpdateRecipeRequest**](UpdateRecipeRequest.md)|  | 

### Return type

void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Recipe updated successfully |  -  |
**400** | Invalid request |  -  |
**401** | Unauthorized |  -  |
**404** | Recipe not found |  -  |
**409** | Recipe was modified concurrently |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

