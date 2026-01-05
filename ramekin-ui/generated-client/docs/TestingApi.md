# TestingApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**ping**](TestingApi.md#ping) | **GET** /api/test/ping |  |
| [**unauthedPing**](TestingApi.md#unauthedping) | **GET** /api/test/unauthed-ping |  |



## ping

> PingResponse ping()



### Example

```ts
import {
  Configuration,
  TestingApi,
} from '';
import type { PingRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new TestingApi(config);

  try {
    const data = await api.ping();
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

[**PingResponse**](PingResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Authenticated ping response |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## unauthedPing

> UnauthedPingResponse unauthedPing()



### Example

```ts
import {
  Configuration,
  TestingApi,
} from '';
import type { UnauthedPingRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const api = new TestingApi();

  try {
    const data = await api.unauthedPing();
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

[**UnauthedPingResponse**](UnauthedPingResponse.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Unauthed ping response |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

