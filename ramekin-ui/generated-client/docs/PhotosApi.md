# PhotosApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**getPhoto**](PhotosApi.md#getphoto) | **GET** /api/photos/{id} |  |
| [**getPhotoThumbnail**](PhotosApi.md#getphotothumbnail) | **GET** /api/photos/{id}/thumbnail |  |
| [**upload**](PhotosApi.md#upload) | **POST** /api/photos |  |



## getPhoto

> getPhoto(id)



### Example

```ts
import {
  Configuration,
  PhotosApi,
} from '';
import type { GetPhotoRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new PhotosApi(config);

  const body = {
    // string | Photo ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
  } satisfies GetPhotoRequest;

  try {
    const data = await api.getPhoto(body);
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
| **id** | `string` | Photo ID | [Defaults to `undefined`] |

### Return type

`void` (Empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/octet-stream`, `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Photo data |  -  |
| **401** | Unauthorized |  -  |
| **404** | Photo not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## getPhotoThumbnail

> getPhotoThumbnail(id)



### Example

```ts
import {
  Configuration,
  PhotosApi,
} from '';
import type { GetPhotoThumbnailRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new PhotosApi(config);

  const body = {
    // string | Photo ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
  } satisfies GetPhotoThumbnailRequest;

  try {
    const data = await api.getPhotoThumbnail(body);
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
| **id** | `string` | Photo ID | [Defaults to `undefined`] |

### Return type

`void` (Empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `image/jpeg`, `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Photo thumbnail data |  -  |
| **401** | Unauthorized |  -  |
| **404** | Photo not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## upload

> UploadPhotoResponse upload(file)



### Example

```ts
import {
  Configuration,
  PhotosApi,
} from '';
import type { UploadRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new PhotosApi(config);

  const body = {
    // Blob
    file: BINARY_DATA_HERE,
  } satisfies UploadRequest;

  try {
    const data = await api.upload(body);
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
| **file** | `Blob` |  | [Defaults to `undefined`] |

### Return type

[**UploadPhotoResponse**](UploadPhotoResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `multipart/form-data`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **201** | Photo uploaded successfully |  -  |
| **400** | Invalid request |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

