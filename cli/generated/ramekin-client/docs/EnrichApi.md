# \EnrichApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**enrich_recipe**](EnrichApi.md#enrich_recipe) | **POST** /api/enrich | Enrich a recipe using AI
[**list_enrichments**](EnrichApi.md#list_enrichments) | **GET** /api/enrichments | List available enrichment types



## enrich_recipe

> models::RecipeContent enrich_recipe(enrich_request)
Enrich a recipe using AI

This is a stateless endpoint that takes a recipe object and returns an enriched version. It does NOT modify any database records. The client can apply the enriched data via a normal PUT /api/recipes/{id} call.

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**enrich_request** | [**EnrichRequest**](EnrichRequest.md) |  | [required] |

### Return type

[**models::RecipeContent**](RecipeContent.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## list_enrichments

> models::ListEnrichmentsResponse list_enrichments()
List available enrichment types

Returns information about all available enrichment types, including their names, descriptions, and which recipe fields they modify.

### Parameters

This endpoint does not need any parameter.

### Return type

[**models::ListEnrichmentsResponse**](ListEnrichmentsResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

