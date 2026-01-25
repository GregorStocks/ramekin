# EnrichApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**enrichRecipe**](EnrichApi.md#enrichrecipe) | **POST** /api/enrich | Enrich a recipe using AI |
| [**listEnrichments**](EnrichApi.md#listenrichments) | **GET** /api/enrichments | List available enrichment types |



## enrichRecipe

> RecipeContent enrichRecipe(enrichRequest)

Enrich a recipe using AI

This is a stateless endpoint that takes a recipe object and returns an enriched version. It does NOT modify any database records. The client can apply the enriched data via a normal PUT /api/recipes/{id} call.

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
    // EnrichRequest
    enrichRequest: ...,
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
| **enrichRequest** | [EnrichRequest](EnrichRequest.md) |  | |

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
| **400** | Invalid enrichment type |  -  |
| **401** | Unauthorized |  -  |
| **503** | AI service unavailable |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## listEnrichments

> ListEnrichmentsResponse listEnrichments()

List available enrichment types

Returns information about all available enrichment types, including their names, descriptions, and which recipe fields they modify.

### Example

```ts
import {
  Configuration,
  EnrichApi,
} from '';
import type { ListEnrichmentsRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new EnrichApi(config);

  try {
    const data = await api.listEnrichments();
    console.log(data);
  } catch (error) {
    console.error(error);
  }
}

// Run the test
example().catch(console.error);
```

### Parameters

This endpoint does not need any parameter.

### Return type

[**ListEnrichmentsResponse**](ListEnrichmentsResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | List of available enrichments |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

