# \RecipesApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_recipe**](RecipesApi.md#create_recipe) | **POST** /api/recipes | 
[**delete_recipe**](RecipesApi.md#delete_recipe) | **DELETE** /api/recipes/{id} | 
[**export_all_recipes**](RecipesApi.md#export_all_recipes) | **GET** /api/recipes/export | 
[**export_recipe**](RecipesApi.md#export_recipe) | **GET** /api/recipes/{id}/export | 
[**get_recipe**](RecipesApi.md#get_recipe) | **GET** /api/recipes/{id} | 
[**list_recipes**](RecipesApi.md#list_recipes) | **GET** /api/recipes | 
[**list_versions**](RecipesApi.md#list_versions) | **GET** /api/recipes/{id}/versions | 
[**rescrape**](RecipesApi.md#rescrape) | **POST** /api/recipes/{id}/rescrape | 
[**update_recipe**](RecipesApi.md#update_recipe) | **PUT** /api/recipes/{id} | 



## create_recipe

> models::CreateRecipeResponse create_recipe(create_recipe_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**create_recipe_request** | [**CreateRecipeRequest**](CreateRecipeRequest.md) |  | [required] |

### Return type

[**models::CreateRecipeResponse**](CreateRecipeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## delete_recipe

> delete_recipe(id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Recipe ID | [required] |

### Return type

 (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## export_all_recipes

> export_all_recipes()


### Parameters

This endpoint does not need any parameter.

### Return type

 (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/zip, application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## export_recipe

> export_recipe(id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Recipe ID | [required] |

### Return type

 (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/gzip, application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_recipe

> models::RecipeResponse get_recipe(id, version_id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Recipe ID | [required] |
**version_id** | Option<**uuid::Uuid**> | Optional version ID to fetch a specific version instead of current |  |

### Return type

[**models::RecipeResponse**](RecipeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## list_recipes

> models::ListRecipesResponse list_recipes(limit, offset, q, sort_by, sort_dir)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**limit** | Option<**i64**> | Number of items to return (default: 20, max: 1000) |  |
**offset** | Option<**i64**> | Number of items to skip (default: 0) |  |
**q** | Option<**String**> | Search query with optional filters. Supports: - Plain text: searches title and description - tag:value: filter by tag (can use multiple) - source:value: filter by source name - has:photos / no:photos: filter by photo presence - created:>2024-01-01: created after date - created:<2024-12-31: created before date - created:2024-01-01..2024-12-31: created in date range  Example: \"chicken tag:dinner tag:quick has:photos\" |  |
**sort_by** | Option<[**SortBy**](.md)> | Sort field (default: updated_at) |  |
**sort_dir** | Option<[**Direction**](.md)> | Sort direction (default: desc). Ignored when sort_by=random. |  |

### Return type

[**models::ListRecipesResponse**](ListRecipesResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## list_versions

> models::VersionListResponse list_versions(id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Recipe ID | [required] |

### Return type

[**models::VersionListResponse**](VersionListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## rescrape

> models::RescrapeResponse rescrape(id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Recipe ID | [required] |

### Return type

[**models::RescrapeResponse**](RescrapeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## update_recipe

> update_recipe(id, update_recipe_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Recipe ID | [required] |
**update_recipe_request** | [**UpdateRecipeRequest**](UpdateRecipeRequest.md) |  | [required] |

### Return type

 (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

