# ClientLogsApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**createClientLog**](ClientLogsApi.md#createclientlogoperation) | **POST** /api/client-logs |  |
| [**getClientLog**](ClientLogsApi.md#getclientlog) | **GET** /api/client-logs/{id} |  |
| [**listClientLogs**](ClientLogsApi.md#listclientlogs) | **GET** /api/client-logs |  |



## createClientLog

> CreateClientLogResponse createClientLog(createClientLogRequest)



### Example

```ts
import {
  Configuration,
  ClientLogsApi,
} from '';
import type { CreateClientLogOperationRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new ClientLogsApi(config);

  const body = {
    // CreateClientLogRequest
    createClientLogRequest: ...,
  } satisfies CreateClientLogOperationRequest;

  try {
    const data = await api.createClientLog(body);
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
| **createClientLogRequest** | [CreateClientLogRequest](CreateClientLogRequest.md) |  | |

### Return type

[**CreateClientLogResponse**](CreateClientLogResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **201** | Log upload stored |  -  |
| **400** | Invalid request |  -  |
| **401** | Unauthorized |  -  |
| **413** | Content too large |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## getClientLog

> GetClientLogResponse getClientLog(id)



### Example

```ts
import {
  Configuration,
  ClientLogsApi,
} from '';
import type { GetClientLogRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new ClientLogsApi(config);

  const body = {
    // string | Log upload id
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
  } satisfies GetClientLogRequest;

  try {
    const data = await api.getClientLog(body);
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
| **id** | `string` | Log upload id | [Defaults to `undefined`] |

### Return type

[**GetClientLogResponse**](GetClientLogResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Full log upload |  -  |
| **401** | Unauthorized |  -  |
| **404** | Not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## listClientLogs

> ListClientLogsResponse listClientLogs()



### Example

```ts
import {
  Configuration,
  ClientLogsApi,
} from '';
import type { ListClientLogsRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new ClientLogsApi(config);

  try {
    const data = await api.listClientLogs();
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

[**ListClientLogsResponse**](ListClientLogsResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Caller\&#39;s log uploads, newest first |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

