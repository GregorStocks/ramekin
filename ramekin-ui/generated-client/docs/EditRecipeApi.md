# EditRecipeApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**editRecipeWithAi**](EditRecipeApi.md#editrecipewithai) | **POST** /api/edit-recipe | Edit a recipe with AI |



## editRecipeWithAi

> RecipeContent editRecipeWithAi(editRecipeRequest)

Edit a recipe with AI

Takes a recipe and a free-text instruction describing the desired change. Returns the complete modified recipe. Stateless - does NOT modify any database records.

### Example

```ts
import {
  Configuration,
  EditRecipeApi,
} from '';
import type { EditRecipeWithAiRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new EditRecipeApi(config);

  const body = {
    // EditRecipeRequest
    editRecipeRequest: ...,
  } satisfies EditRecipeWithAiRequest;

  try {
    const data = await api.editRecipeWithAi(body);
    console.log(data);
  } catch (error) {
    console.error(error);
  }
}

// Run the test
example().catch(console.error);
```

### Parameters


| Name | Type | Description  | Notes |
|------------- | ------------- | ------------- | -------------|
| **editRecipeRequest** | [EditRecipeRequest](EditRecipeRequest.md) |  | |

### Return type

[**RecipeContent**](RecipeContent.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Modified recipe |  -  |
| **400** | Invalid photo IDs |  -  |
| **401** | Unauthorized |  -  |
| **500** | Internal server error |  -  |
| **503** | AI service unavailable |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

