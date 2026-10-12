# IngredientNamesApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**getIngredientNamesStatus**](IngredientNamesApi.md#getingredientnamesstatus) | **GET** /api/ingredient-names/status |  |
| [**retryIngredientNames**](IngredientNamesApi.md#retryingredientnames) | **POST** /api/ingredient-names/retry |  |



## getIngredientNamesStatus

> IngredientNamesStatusResponse getIngredientNamesStatus()



### Example

```ts
import {
  Configuration,
  IngredientNamesApi,
} from '';
import type { GetIngredientNamesStatusRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new IngredientNamesApi(config);

  try {
    const data = await api.getIngredientNamesStatus();
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

[**IngredientNamesStatusResponse**](IngredientNamesStatusResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Resolution status of ingredient names the catalog doesn\&#39;t know |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## retryIngredientNames

> IngredientNamesQueuedResponse retryIngredientNames()



### Example

```ts
import {
  Configuration,
  IngredientNamesApi,
} from '';
import type { RetryIngredientNamesRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new IngredientNamesApi(config);

  try {
    const data = await api.retryIngredientNames();
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

[**IngredientNamesQueuedResponse**](IngredientNamesQueuedResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Failed names queued again |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

