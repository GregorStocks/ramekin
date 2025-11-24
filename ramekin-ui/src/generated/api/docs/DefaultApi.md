# DefaultApi

All URIs are relative to _http://localhost_

| Method                                       | HTTP request          | Description |
| -------------------------------------------- | --------------------- | ----------- |
| [**getGarbages**](DefaultApi.md#getgarbages) | **GET** /api/garbages |             |

## getGarbages

> GarbagesResponse getGarbages()

### Example

```ts
import { Configuration, DefaultApi } from "";
import type { GetGarbagesRequest } from "";

async function example() {
  console.log("🚀 Testing  SDK...");
  const api = new DefaultApi();

  try {
    const data = await api.getGarbages();
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

[**GarbagesResponse**](GarbagesResponse.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`

### HTTP response details

| Status code | Description          | Response headers |
| ----------- | -------------------- | ---------------- |
| **200**     | List of all garbages | -                |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)
