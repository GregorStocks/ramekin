# SyncApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**syncMealPlans**](SyncApi.md#syncmealplansoperation) | **POST** /api/sync/meal-plans |  |
| [**syncRecipes**](SyncApi.md#syncrecipes) | **GET** /api/sync/recipes |  |
| [**syncShoppingList**](SyncApi.md#syncshoppinglistoperation) | **POST** /api/sync/shopping-list |  |
| [**syncTags**](SyncApi.md#synctags) | **GET** /api/sync/tags |  |



## syncMealPlans

> SyncMealPlansResponse syncMealPlans(syncMealPlansRequest)



### Example

```ts
import {
  Configuration,
  SyncApi,
} from '';
import type { SyncMealPlansOperationRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new SyncApi(config);

  const body = {
    // SyncMealPlansRequest
    syncMealPlansRequest: ...,
  } satisfies SyncMealPlansOperationRequest;

  try {
    const data = await api.syncMealPlans(body);
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
| **syncMealPlansRequest** | [SyncMealPlansRequest](SyncMealPlansRequest.md) |  | |

### Return type

[**SyncMealPlansResponse**](SyncMealPlansResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Ops applied; changes since the cursor |  -  |
| **400** | A malformed op |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## syncRecipes

> SyncRecipesResponse syncRecipes(limit, cursor, afterId)



### Example

```ts
import {
  Configuration,
  SyncApi,
} from '';
import type { SyncRecipesRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new SyncApi(config);

  const body = {
    // number | Maximum number of recipes returned per page, between 1 and 500.
    limit: 789,
    // number | Cursor returned by a previous sync. Absent means a full sync. Every page of one sweep passes the same cursor. (optional)
    cursor: 789,
    // string | ID of the last recipe on the previous page. Absent on a sweep\'s first page. (optional)
    afterId: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
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
| **limit** | `number` | Maximum number of recipes returned per page, between 1 and 500. | [Defaults to `undefined`] |
| **cursor** | `number` | Cursor returned by a previous sync. Absent means a full sync. Every page of one sweep passes the same cursor. | [Optional] [Defaults to `undefined`] |
| **afterId** | `string` | ID of the last recipe on the previous page. Absent on a sweep\&#39;s first page. | [Optional] [Defaults to `undefined`] |

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
| **400** | Invalid limit |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## syncShoppingList

> SyncShoppingListResponse syncShoppingList(syncShoppingListRequest)



### Example

```ts
import {
  Configuration,
  SyncApi,
} from '';
import type { SyncShoppingListOperationRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new SyncApi(config);

  const body = {
    // SyncShoppingListRequest
    syncShoppingListRequest: ...,
  } satisfies SyncShoppingListOperationRequest;

  try {
    const data = await api.syncShoppingList(body);
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
| **syncShoppingListRequest** | [SyncShoppingListRequest](SyncShoppingListRequest.md) |  | |

### Return type

[**SyncShoppingListResponse**](SyncShoppingListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Ops applied; changes since the cursor |  -  |
| **400** | A malformed op |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## syncTags

> SyncTagsResponse syncTags(cursor)



### Example

```ts
import {
  Configuration,
  SyncApi,
} from '';
import type { SyncTagsRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new SyncApi(config);

  const body = {
    // number | Cursor returned by the previous sync. Absent means a full sync. (optional)
    cursor: 789,
  } satisfies SyncTagsRequest;

  try {
    const data = await api.syncTags(body);
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

[**SyncTagsResponse**](SyncTagsResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Tag changes for the local cache |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

