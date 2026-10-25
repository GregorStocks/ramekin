# \EditRecipeApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**edit_recipe_with_ai**](EditRecipeApi.md#edit_recipe_with_ai) | **POST** /api/edit-recipe | Edit a recipe with AI



## edit_recipe_with_ai

> models::RecipeContent edit_recipe_with_ai(edit_recipe_request)
Edit a recipe with AI

Takes a recipe and a free-text instruction describing the desired change. Returns the complete modified recipe. Stateless - does NOT modify any database records.

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**edit_recipe_request** | [**EditRecipeRequest**](EditRecipeRequest.md) |  | [required] |

### Return type

[**models::RecipeContent**](RecipeContent.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

