# \EditRecipeApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**edit_recipe_with_ai**](EditRecipeApi.md#edit_recipe_with_ai) | **POST** /api/recipes/{id}/edit-with-ai | Edit a recipe with AI



## edit_recipe_with_ai

> models::NormalizeJobResponse edit_recipe_with_ai(id, edit_recipe_request)
Edit a recipe with AI

Starts a job that edits the recipe's current version the way the instruction says, looking up catalog weights and calories and the user's tags and recipes as it goes. Poll it with `GET /api/scrape/{id}`; once it completes, `GET /api/recipes/{id}/normalize/{job_id}` returns the proposed version and a summary of the changes, and `POST /api/recipes/{id}/normalize/{job_id}/accept` saves it.

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Recipe ID | [required] |
**edit_recipe_request** | [**EditRecipeRequest**](EditRecipeRequest.md) |  | [required] |

### Return type

[**models::NormalizeJobResponse**](NormalizeJobResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

