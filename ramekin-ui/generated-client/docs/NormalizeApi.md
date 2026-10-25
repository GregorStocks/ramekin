# NormalizeApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**acceptNormalizeProposal**](NormalizeApi.md#acceptnormalizeproposal) | **POST** /api/recipes/{id}/normalize/{job_id}/accept |  |
| [**getNormalizeProposal**](NormalizeApi.md#getnormalizeproposal) | **GET** /api/recipes/{id}/normalize/{job_id} |  |
| [**listNormalizeOps**](NormalizeApi.md#listnormalizeops) | **GET** /api/recipes/normalize-ops |  |
| [**normalizeRecipe**](NormalizeApi.md#normalizerecipe) | **POST** /api/recipes/{id}/normalize |  |
| [**normalizeRecipes**](NormalizeApi.md#normalizerecipes) | **POST** /api/recipes/normalize |  |



## acceptNormalizeProposal

> AcceptNormalizeResponse acceptNormalizeProposal(id, jobId)



### Example

```ts
import {
  Configuration,
  NormalizeApi,
} from '';
import type { AcceptNormalizeProposalRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new NormalizeApi(config);

  const body = {
    // string | Recipe ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
    // string | Normalize job ID
    jobId: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
  } satisfies AcceptNormalizeProposalRequest;

  try {
    const data = await api.acceptNormalizeProposal(body);
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
| **jobId** | `string` | Normalize job ID | [Defaults to `undefined`] |

### Return type

[**AcceptNormalizeResponse**](AcceptNormalizeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **201** | Proposal saved as the recipe\&#39;s new version |  -  |
| **400** | The job has not finished proposing a version |  -  |
| **401** | Unauthorized |  -  |
| **404** | Normalize job not found |  -  |
| **409** | The recipe changed since the job started |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## getNormalizeProposal

> NormalizeProposalResponse getNormalizeProposal(id, jobId)



### Example

```ts
import {
  Configuration,
  NormalizeApi,
} from '';
import type { GetNormalizeProposalRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new NormalizeApi(config);

  const body = {
    // string | Recipe ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
    // string | Normalize job ID
    jobId: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
  } satisfies GetNormalizeProposalRequest;

  try {
    const data = await api.getNormalizeProposal(body);
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
| **jobId** | `string` | Normalize job ID | [Defaults to `undefined`] |

### Return type

[**NormalizeProposalResponse**](NormalizeProposalResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | The job\&#39;s base version and, once it completes, its proposal |  -  |
| **401** | Unauthorized |  -  |
| **404** | Normalize job not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## listNormalizeOps

> NormalizeOpsResponse listNormalizeOps(recipeId)



### Example

```ts
import {
  Configuration,
  NormalizeApi,
} from '';
import type { ListNormalizeOpsRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new NormalizeApi(config);

  const body = {
    // string | Report which ops can run on this recipe. (optional)
    recipeId: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
  } satisfies ListNormalizeOpsRequest;

  try {
    const data = await api.listNormalizeOps(body);
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
| **recipeId** | `string` | Report which ops can run on this recipe. | [Optional] [Defaults to `undefined`] |

### Return type

[**NormalizeOpsResponse**](NormalizeOpsResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Every normalize op, in run order |  -  |
| **401** | Unauthorized |  -  |
| **404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## normalizeRecipe

> NormalizeJobResponse normalizeRecipe(id, normalizeRequest)



### Example

```ts
import {
  Configuration,
  NormalizeApi,
} from '';
import type { NormalizeRecipeRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new NormalizeApi(config);

  const body = {
    // string | Recipe ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
    // NormalizeRequest
    normalizeRequest: ...,
  } satisfies NormalizeRecipeRequest;

  try {
    const data = await api.normalizeRecipe(body);
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
| **normalizeRequest** | [NormalizeRequest](NormalizeRequest.md) |  | |

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
| **201** | Normalize job started; it proposes a version |  -  |
| **400** | Unknown op, or an op unavailable for this recipe |  -  |
| **401** | Unauthorized |  -  |
| **404** | Recipe not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## normalizeRecipes

> BulkNormalizeResponse normalizeRecipes(bulkNormalizeRequest)



### Example

```ts
import {
  Configuration,
  NormalizeApi,
} from '';
import type { NormalizeRecipesRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new NormalizeApi(config);

  const body = {
    // BulkNormalizeRequest
    bulkNormalizeRequest: ...,
  } satisfies NormalizeRecipesRequest;

  try {
    const data = await api.normalizeRecipes(body);
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
| **bulkNormalizeRequest** | [BulkNormalizeRequest](BulkNormalizeRequest.md) |  | |

### Return type

[**BulkNormalizeResponse**](BulkNormalizeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | One result per recipe: its job, or why none started |  -  |
| **400** | Unknown op |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

