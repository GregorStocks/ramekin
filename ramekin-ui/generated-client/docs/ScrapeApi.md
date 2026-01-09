# ScrapeApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**capture**](ScrapeApi.md#captureoperation) | **POST** /api/scrape/capture |  |
| [**createScrape**](ScrapeApi.md#createscrapeoperation) | **POST** /api/scrape |  |
| [**getScrape**](ScrapeApi.md#getscrape) | **GET** /api/scrape/{id} |  |
| [**retryScrape**](ScrapeApi.md#retryscrape) | **POST** /api/scrape/{id}/retry |  |



## capture

> CaptureResponse capture(captureRequest)



### Example

```ts
import {
  Configuration,
  ScrapeApi,
} from '';
import type { CaptureOperationRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new ScrapeApi(config);

  const body = {
    // CaptureRequest
    captureRequest: ...,
  } satisfies CaptureOperationRequest;

  try {
    const data = await api.capture(body);
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
| **captureRequest** | [CaptureRequest](CaptureRequest.md) |  | |

### Return type

[**CaptureResponse**](CaptureResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **201** | Recipe created from captured HTML |  -  |
| **400** | Invalid URL or no recipe found |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## createScrape

> CreateScrapeResponse createScrape(createScrapeRequest)



### Example

```ts
import {
  Configuration,
  ScrapeApi,
} from '';
import type { CreateScrapeOperationRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new ScrapeApi(config);

  const body = {
    // CreateScrapeRequest
    createScrapeRequest: ...,
  } satisfies CreateScrapeOperationRequest;

  try {
    const data = await api.createScrape(body);
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
| **createScrapeRequest** | [CreateScrapeRequest](CreateScrapeRequest.md) |  | |

### Return type

[**CreateScrapeResponse**](CreateScrapeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **201** | Scrape job created |  -  |
| **400** | Invalid URL |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## getScrape

> ScrapeJobResponse getScrape(id)



### Example

```ts
import {
  Configuration,
  ScrapeApi,
} from '';
import type { GetScrapeRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new ScrapeApi(config);

  const body = {
    // string | Scrape job ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
  } satisfies GetScrapeRequest;

  try {
    const data = await api.getScrape(body);
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
| **id** | `string` | Scrape job ID | [Defaults to `undefined`] |

### Return type

[**ScrapeJobResponse**](ScrapeJobResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Scrape job status |  -  |
| **401** | Unauthorized |  -  |
| **404** | Job not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## retryScrape

> RetryScrapeResponse retryScrape(id)



### Example

```ts
import {
  Configuration,
  ScrapeApi,
} from '';
import type { RetryScrapeRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new ScrapeApi(config);

  const body = {
    // string | Scrape job ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
  } satisfies RetryScrapeRequest;

  try {
    const data = await api.retryScrape(body);
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
| **id** | `string` | Scrape job ID | [Defaults to `undefined`] |

### Return type

[**RetryScrapeResponse**](RetryScrapeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Retry initiated |  -  |
| **400** | Cannot retry job |  -  |
| **401** | Unauthorized |  -  |
| **404** | Job not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

