# \EnrichApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**custom_enrich_recipe**](EnrichApi.md#custom_enrich_recipe) | **POST** /api/enrich/custom | Apply a custom AI modification to a recipe
[**enrich_recipe**](EnrichApi.md#enrich_recipe) | **POST** /api/enrich | Enrich a recipe using AI



## custom_enrich_recipe

> models::RecipeContent custom_enrich_recipe(custom_enrich_request)
Apply a custom AI modification to a recipe

Takes a recipe and a free-text instruction describing the desired change. Returns the complete modified recipe. Stateless - does NOT modify any database records.

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**custom_enrich_request** | [**CustomEnrichRequest**](CustomEnrichRequest.md) |  | [required] |

### Return type

[**models::RecipeContent**](RecipeContent.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## enrich_recipe

> models::RecipeContent enrich_recipe(recipe_content)
Enrich a recipe using AI

This is a stateless endpoint that takes a recipe object and returns an enriched version. It does NOT modify any database records. The client can apply the enriched data via a normal PUT /api/recipes/{id} call.  Currently enriches tags by suggesting from the user's existing tag library.

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**recipe_content** | [**RecipeContent**](RecipeContent.md) |  | [required] |

### Return type

[**models::RecipeContent**](RecipeContent.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

