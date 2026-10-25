# EditRecipeApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**editRecipeWithAi**](EditRecipeApi.md#editrecipewithai) | **POST** /api/recipes/{id}/edit-with-ai | Edit a recipe with AI |



## editRecipeWithAi

> NormalizeJobResponse editRecipeWithAi(id, editRecipeRequest)

Edit a recipe with AI

Starts a job that edits the recipe\&#39;s current version the way the instruction says, looking up catalog weights and calories and the user\&#39;s tags and recipes as it goes. Poll it with &#x60;GET /api/scrape/{id}&#x60;; once it completes, &#x60;GET /api/recipes/{id}/normalize/{job_id}&#x60; returns the proposed version and a summary of the changes, and &#x60;POST /api/recipes/{id}/normalize/{job_id}/accept&#x60; saves it.

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
    // string | Recipe ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
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
| **id** | `string` | Recipe ID | [Defaults to `undefined`] |
| **editRecipeRequest** | [EditRecipeRequest](EditRecipeRequest.md) |  | |

### Return type

[**NormalizeJobResponse**](NormalizeJobResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **201** | Edit job started; it proposes a version |  -  |
| **400** | Empty instruction, or invalid photo IDs |  -  |
| **401** | Unauthorized |  -  |
| **404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

