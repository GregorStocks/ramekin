# RecipesApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**createRecipe**](RecipesApi.md#createrecipeoperation) | **POST** /api/recipes |  |
| [**deleteRecipe**](RecipesApi.md#deleterecipe) | **DELETE** /api/recipes/{id} |  |
| [**exportAllRecipes**](RecipesApi.md#exportallrecipes) | **GET** /api/recipes/export |  |
| [**exportRecipe**](RecipesApi.md#exportrecipe) | **GET** /api/recipes/{id}/export |  |
| [**generateDescription**](RecipesApi.md#generatedescription) | **POST** /api/recipes/{id}/generate-description |  |
| [**generatePhoto**](RecipesApi.md#generatephoto) | **POST** /api/recipes/{id}/generate-photo |  |
| [**getRecipe**](RecipesApi.md#getrecipe) | **GET** /api/recipes/{id} |  |
| [**listRecipes**](RecipesApi.md#listrecipes) | **GET** /api/recipes |  |
| [**listVersions**](RecipesApi.md#listversions) | **GET** /api/recipes/{id}/versions |  |
| [**normalizeTitle**](RecipesApi.md#normalizetitle) | **POST** /api/recipes/{id}/normalize-title |  |
| [**rescrape**](RecipesApi.md#rescrape) | **POST** /api/recipes/{id}/rescrape |  |
| [**rescrapePhoto**](RecipesApi.md#rescrapephoto) | **POST** /api/recipes/{id}/rescrape-photo |  |
| [**syncRecipes**](RecipesApi.md#syncrecipes) | **GET** /api/recipes/sync |  |
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


## exportAllRecipes

> exportAllRecipes()



### Example

```ts
import {
  Configuration,
  RecipesApi,
} from '';
import type { ExportAllRecipesRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new RecipesApi(config);

  try {
    const data = await api.exportAllRecipes();
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

`void` (Empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/zip`, `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Paprika recipes archive (.paprikarecipes) |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## exportRecipe

> exportRecipe(id)



### Example

```ts
import {
  Configuration,
  RecipesApi,
} from '';
import type { ExportRecipeRequest } from '';

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
  } satisfies ExportRecipeRequest;

  try {
    const data = await api.exportRecipe(body);
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
- **Accept**: `application/gzip`, `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Paprika recipe file (.paprikarecipe) |  -  |
| **401** | Unauthorized |  -  |
| **404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## generateDescription

> GenerateDescriptionResponse generateDescription(id)



### Example

```ts
import {
  Configuration,
  RecipesApi,
} from '';
import type { GenerateDescriptionRequest } from '';

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
  } satisfies GenerateDescriptionRequest;

  try {
    const data = await api.generateDescription(body);
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

[**GenerateDescriptionResponse**](GenerateDescriptionResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Description generated and applied |  -  |
| **401** | Unauthorized |  -  |
| **404** | Recipe not found |  -  |
| **409** | Recipe was modified concurrently |  -  |
| **503** | AI service unavailable |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## generatePhoto

> GeneratePhotoResponse generatePhoto(id)



### Example

```ts
import {
  Configuration,
  RecipesApi,
} from '';
import type { GeneratePhotoRequest } from '';

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
  } satisfies GeneratePhotoRequest;

  try {
    const data = await api.generatePhoto(body);
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

[**GeneratePhotoResponse**](GeneratePhotoResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Recipe photo generated and applied |  -  |
| **401** | Unauthorized |  -  |
| **404** | Recipe not found |  -  |
| **409** | Recipe changed during generation |  -  |
| **503** | AI service unavailable |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## getRecipe

> RecipeResponse getRecipe(id, versionId)



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
    // string | Optional version ID to fetch a specific version instead of current (optional)
    versionId: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
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
| **versionId** | `string` | Optional version ID to fetch a specific version instead of current | [Optional] [Defaults to `undefined`] |

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
    // string | Search query with optional filters. Supports: - Plain text: searches title and description - tag:value: filter by tag (can use multiple) - source:value: filter by source name - has:photos / no:photos: filter by photo presence - created:>2024-01-01: created on or after date - created:<2024-12-31: created on or before date - created:2024-01-01..2024-12-31: created in date range  Date filters name inclusive UTC calendar days.  Example: \"chicken tag:dinner tag:quick has:photos\" (optional)
    q: q_example,
    // SortBy | Sort field. Defaults to relevance when the query has text terms, otherwise updated_at. (optional)
    sortBy: ...,
    // Direction | Sort direction (default: desc). Ignored when sort_by is random or relevance. (optional)
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
| **q** | `string` | Search query with optional filters. Supports: - Plain text: searches title and description - tag:value: filter by tag (can use multiple) - source:value: filter by source name - has:photos / no:photos: filter by photo presence - created:&gt;2024-01-01: created on or after date - created:&lt;2024-12-31: created on or before date - created:2024-01-01..2024-12-31: created in date range  Date filters name inclusive UTC calendar days.  Example: \&quot;chicken tag:dinner tag:quick has:photos\&quot; | [Optional] [Defaults to `undefined`] |
| **sortBy** | `SortBy` | Sort field. Defaults to relevance when the query has text terms, otherwise updated_at. | [Optional] [Defaults to `undefined`] [Enum: relevance, updated_at, rating, title, created_at, random] |
| **sortDir** | `Direction` | Sort direction (default: desc). Ignored when sort_by is random or relevance. | [Optional] [Defaults to `undefined`] [Enum: desc, asc] |

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


## listVersions

> VersionListResponse listVersions(id)



### Example

```ts
import {
  Configuration,
  RecipesApi,
} from '';
import type { ListVersionsRequest } from '';

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
  } satisfies ListVersionsRequest;

  try {
    const data = await api.listVersions(body);
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

[**VersionListResponse**](VersionListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | List of recipe versions |  -  |
| **401** | Unauthorized |  -  |
| **404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## normalizeTitle

> NormalizeTitleResponse normalizeTitle(id)



### Example

```ts
import {
  Configuration,
  RecipesApi,
} from '';
import type { NormalizeTitleRequest } from '';

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
  } satisfies NormalizeTitleRequest;

  try {
    const data = await api.normalizeTitle(body);
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

[**NormalizeTitleResponse**](NormalizeTitleResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Title normalized and applied |  -  |
| **401** | Unauthorized |  -  |
| **404** | Recipe not found |  -  |
| **409** | Recipe was modified concurrently |  -  |
| **503** | AI service unavailable |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## rescrape

> RescrapeResponse rescrape(id)



### Example

```ts
import {
  Configuration,
  RecipesApi,
} from '';
import type { RescrapeRequest } from '';

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
  } satisfies RescrapeRequest;

  try {
    const data = await api.rescrape(body);
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

[**RescrapeResponse**](RescrapeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **201** | Rescrape job created |  -  |
| **400** | Recipe has no source URL |  -  |
| **401** | Unauthorized |  -  |
| **404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## rescrapePhoto

> RescrapeResponse rescrapePhoto(id)



### Example

```ts
import {
  Configuration,
  RecipesApi,
} from '';
import type { RescrapePhotoRequest } from '';

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
  } satisfies RescrapePhotoRequest;

  try {
    const data = await api.rescrapePhoto(body);
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

[**RescrapeResponse**](RescrapeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **201** | Photo rescrape job created |  -  |
| **400** | Recipe has no source URL |  -  |
| **401** | Unauthorized |  -  |
| **404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## syncRecipes

> SyncRecipesResponse syncRecipes(cursor)



### Example

```ts
import {
  Configuration,
  RecipesApi,
} from '';
import type { SyncRecipesRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new RecipesApi(config);

  const body = {
    // number | Cursor returned by the previous sync. Absent means a full sync. (optional)
    cursor: 789,
  } satisfies SyncRecipesRequest;

  try {
    const data = await api.syncRecipes(body);
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
| **cursor** | `number` | Cursor returned by the previous sync. Absent means a full sync. | [Optional] [Defaults to `undefined`] |

### Return type

[**SyncRecipesResponse**](SyncRecipesResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Recipe changes for local cache sync |  -  |
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
| **409** | Recipe was modified concurrently |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

