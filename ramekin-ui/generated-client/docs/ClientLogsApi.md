# ClientLogsApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**createClientLog**](ClientLogsApi.md#createclientlogoperation) | **POST** /api/client-logs |  |



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

