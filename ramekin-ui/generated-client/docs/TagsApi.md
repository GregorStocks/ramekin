# TagsApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**createTag**](TagsApi.md#createtagoperation) | **POST** /api/tags |  |
| [**deleteTag**](TagsApi.md#deletetag) | **DELETE** /api/tags/{id} |  |
| [**listAllTags**](TagsApi.md#listalltags) | **GET** /api/tags |  |
| [**renameTag**](TagsApi.md#renametagoperation) | **PATCH** /api/tags/{id} |  |



## createTag

> CreateTagResponse createTag(createTagRequest)



### Example

```ts
import {
  Configuration,
  TagsApi,
} from '';
import type { CreateTagOperationRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new TagsApi(config);

  const body = {
    // CreateTagRequest
    createTagRequest: ...,
  } satisfies CreateTagOperationRequest;

  try {
    const data = await api.createTag(body);
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
| **createTagRequest** | [CreateTagRequest](CreateTagRequest.md) |  | |

### Return type

[**CreateTagResponse**](CreateTagResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **201** | Tag created successfully |  -  |
| **400** | Invalid request (empty name) |  -  |
| **401** | Unauthorized |  -  |
| **409** | Tag already exists |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## deleteTag

> deleteTag(id)



### Example

```ts
import {
  Configuration,
  TagsApi,
} from '';
import type { DeleteTagRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new TagsApi(config);

  const body = {
    // string | Tag ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
  } satisfies DeleteTagRequest;

  try {
    const data = await api.deleteTag(body);
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
| **id** | `string` | Tag ID | [Defaults to `undefined`] |

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
| **204** | Tag deleted successfully |  -  |
| **401** | Unauthorized |  -  |
| **404** | Tag not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## listAllTags

> TagsListResponse listAllTags()



### Example

```ts
import {
  Configuration,
  TagsApi,
} from '';
import type { ListAllTagsRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new TagsApi(config);

  try {
    const data = await api.listAllTags();
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

[**TagsListResponse**](TagsListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | List of user\&#39;s tags with IDs and recipe counts |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## renameTag

> RenameTagResponse renameTag(id, renameTagRequest)



### Example

```ts
import {
  Configuration,
  TagsApi,
} from '';
import type { RenameTagOperationRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new TagsApi(config);

  const body = {
    // string | Tag ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
    // RenameTagRequest
    renameTagRequest: ...,
  } satisfies RenameTagOperationRequest;

  try {
    const data = await api.renameTag(body);
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
| **id** | `string` | Tag ID | [Defaults to `undefined`] |
| **renameTagRequest** | [RenameTagRequest](RenameTagRequest.md) |  | |

### Return type

[**RenameTagResponse**](RenameTagResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Tag renamed successfully |  -  |
| **400** | Invalid request (empty name) |  -  |
| **401** | Unauthorized |  -  |
| **404** | Tag not found |  -  |
| **409** | Tag with that name already exists |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

