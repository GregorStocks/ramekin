# ImportApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**importRecipe**](ImportApi.md#importrecipeoperation) | **POST** /api/import/recipe |  |



## importRecipe

> ImportRecipeResponse importRecipe(importRecipeRequest)



### Example

```ts
import {
  Configuration,
  ImportApi,
} from '';
import type { ImportRecipeOperationRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new ImportApi(config);

  const body = {
    // ImportRecipeRequest
    importRecipeRequest: ...,
  } satisfies ImportRecipeOperationRequest;

  try {
    const data = await api.importRecipe(body);
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
| **importRecipeRequest** | [ImportRecipeRequest](ImportRecipeRequest.md) |  | |

### Return type

[**ImportRecipeResponse**](ImportRecipeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **201** | Import job created |  -  |
| **400** | Invalid request |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

