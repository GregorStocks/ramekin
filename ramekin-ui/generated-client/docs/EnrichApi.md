# EnrichApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**customEnrichRecipe**](EnrichApi.md#customenrichrecipe) | **POST** /api/enrich/custom | Apply a custom AI modification to a recipe |
| [**enrichRecipe**](EnrichApi.md#enrichrecipe) | **POST** /api/enrich | Enrich a recipe |



## customEnrichRecipe

> RecipeContent customEnrichRecipe(customEnrichRequest)

Apply a custom AI modification to a recipe

Takes a recipe and a free-text instruction describing the desired change. Returns the complete modified recipe. Stateless - does NOT modify any database records.

### Example

```ts
import {
  Configuration,
  EnrichApi,
} from '';
import type { CustomEnrichRecipeRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new EnrichApi(config);

  const body = {
    // CustomEnrichRequest
    customEnrichRequest: ...,
  } satisfies CustomEnrichRecipeRequest;

  try {
    const data = await api.customEnrichRecipe(body);
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
| **customEnrichRequest** | [CustomEnrichRequest](CustomEnrichRequest.md) |  | |

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
| **401** | Unauthorized |  -  |
| **503** | AI service unavailable |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## enrichRecipe

> RecipeContent enrichRecipe(recipeContent)

Enrich a recipe

This is a stateless endpoint that takes a recipe object and returns an enriched version. It does NOT modify any database records. The client can apply the enriched data via a normal PUT /api/recipes/{id} call.  Enriches: - Ingredient measurements with gram conversions (volume/weight → grams) - Tags by suggesting from the user\&#39;s existing tag library (requires AI; skipped if unavailable)

### Example

```ts
import {
  Configuration,
  EnrichApi,
} from '';
import type { EnrichRecipeRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new EnrichApi(config);

  const body = {
    // RecipeContent
    recipeContent: ...,
  } satisfies EnrichRecipeRequest;

  try {
    const data = await api.enrichRecipe(body);
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
| **recipeContent** | [RecipeContent](RecipeContent.md) |  | |

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
| **200** | Enriched recipe object |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

