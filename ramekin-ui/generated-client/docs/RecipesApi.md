# RecipesApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**createRecipe**](RecipesApi.md#createrecipeoperation) | **POST** /api/recipes |  |
| [**deleteRecipe**](RecipesApi.md#deleterecipe) | **DELETE** /api/recipes/{id} |  |
| [**getRecipe**](RecipesApi.md#getrecipe) | **GET** /api/recipes/{id} |  |
| [**listRecipes**](RecipesApi.md#listrecipes) | **GET** /api/recipes |  |
| [**listTags**](RecipesApi.md#listtags) | **GET** /api/recipes/tags |  |
| [**updateRecipe**](RecipesApi.md#updaterecipeoperation) | **PUT** /api/recipes/{id} |  |



## createRecipe

> CreateRecipeResponse createRecipe(createRecipeRequest)



### Example

```ts
import {
  Configuration,
  RecipesApi,
} from '';
import type { CreateRecipeOperationRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new RecipesApi(config);

  const body = {
    // CreateRecipeRequest
    createRecipeRequest: ...,
  } satisfies CreateRecipeOperationRequest;

  try {
    const data = await api.createRecipe(body);
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
| **createRecipeRequest** | [CreateRecipeRequest](CreateRecipeRequest.md) |  | |

### Return type

[**CreateRecipeResponse**](CreateRecipeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **201** | Recipe created successfully |  -  |
| **400** | Invalid request |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## deleteRecipe

> deleteRecipe(id)



### Example

```ts
import {
  Configuration,
  RecipesApi,
} from '';
import type { DeleteRecipeRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new RecipesApi(config);

  const body = {
    // string | Recipe ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
  } satisfies DeleteRecipeRequest;

  try {
    const data = await api.deleteRecipe(body);
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

### Return type

`void` (Empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **204** | Recipe deleted successfully |  -  |
| **401** | Unauthorized |  -  |
| **404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## getRecipe

> RecipeResponse getRecipe(id)



### Example

```ts
import {
  Configuration,
  RecipesApi,
} from '';
import type { GetRecipeRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new RecipesApi(config);

  const body = {
    // string | Recipe ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
  } satisfies GetRecipeRequest;

  try {
    const data = await api.getRecipe(body);
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

### Return type

[**RecipeResponse**](RecipeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Recipe details |  -  |
| **401** | Unauthorized |  -  |
| **404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## listRecipes

> ListRecipesResponse listRecipes(limit, offset, q, sortBy, sortDir)



### Example

```ts
import {
  Configuration,
  RecipesApi,
} from '';
import type { ListRecipesRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new RecipesApi(config);

  const body = {
    // number | Number of items to return (default: 20, max: 1000) (optional)
    limit: 789,
    // number | Number of items to skip (default: 0) (optional)
    offset: 789,
    // string | Search query with optional filters. Supports: - Plain text: searches title and description - tag:value: filter by tag (can use multiple) - source:value: filter by source name - has:photos / no:photos: filter by photo presence - created:>2024-01-01: created after date - created:<2024-12-31: created before date - created:2024-01-01..2024-12-31: created in date range  Example: \"chicken tag:dinner tag:quick has:photos\" (optional)
    q: q_example,
    // SortBy | Sort field (default: updated_at) (optional)
    sortBy: ...,
    // Direction | Sort direction (default: desc). Ignored when sort_by=random. (optional)
    sortDir: ...,
  } satisfies ListRecipesRequest;

  try {
    const data = await api.listRecipes(body);
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
| **limit** | `number` | Number of items to return (default: 20, max: 1000) | [Optional] [Defaults to `undefined`] |
| **offset** | `number` | Number of items to skip (default: 0) | [Optional] [Defaults to `undefined`] |
| **q** | `string` | Search query with optional filters. Supports: - Plain text: searches title and description - tag:value: filter by tag (can use multiple) - source:value: filter by source name - has:photos / no:photos: filter by photo presence - created:&gt;2024-01-01: created after date - created:&lt;2024-12-31: created before date - created:2024-01-01..2024-12-31: created in date range  Example: \&quot;chicken tag:dinner tag:quick has:photos\&quot; | [Optional] [Defaults to `undefined`] |
| **sortBy** | `SortBy` | Sort field (default: updated_at) | [Optional] [Defaults to `undefined`] [Enum: updated_at, random] |
| **sortDir** | `Direction` | Sort direction (default: desc). Ignored when sort_by&#x3D;random. | [Optional] [Defaults to `undefined`] [Enum: desc, asc] |

### Return type

[**ListRecipesResponse**](ListRecipesResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | List of user\&#39;s recipes |  -  |
| **400** | Invalid parameters |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## listTags

> TagsResponse listTags()



### Example

```ts
import {
  Configuration,
  RecipesApi,
} from '';
import type { ListTagsRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new RecipesApi(config);

  try {
    const data = await api.listTags();
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

[**TagsResponse**](TagsResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | List of distinct tags |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## updateRecipe

> updateRecipe(id, updateRecipeRequest)



### Example

```ts
import {
  Configuration,
  RecipesApi,
} from '';
import type { UpdateRecipeOperationRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new RecipesApi(config);

  const body = {
    // string | Recipe ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
    // UpdateRecipeRequest
    updateRecipeRequest: ...,
  } satisfies UpdateRecipeOperationRequest;

  try {
    const data = await api.updateRecipe(body);
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
| **updateRecipeRequest** | [UpdateRecipeRequest](UpdateRecipeRequest.md) |  | |

### Return type

`void` (Empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Recipe updated successfully |  -  |
| **400** | Invalid request |  -  |
| **401** | Unauthorized |  -  |
| **404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

