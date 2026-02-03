# ShoppingListApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**clearChecked**](ShoppingListApi.md#clearchecked) | **DELETE** /api/shopping-list/clear-checked |  |
| [**createItems**](ShoppingListApi.md#createitems) | **POST** /api/shopping-list |  |
| [**deleteItem**](ShoppingListApi.md#deleteitem) | **DELETE** /api/shopping-list/{id} |  |
| [**listItems**](ShoppingListApi.md#listitems) | **GET** /api/shopping-list |  |
| [**syncItems**](ShoppingListApi.md#syncitems) | **POST** /api/shopping-list/sync |  |
| [**updateItem**](ShoppingListApi.md#updateitem) | **PUT** /api/shopping-list/{id} |  |



## clearChecked

> ClearCheckedResponse clearChecked()



### Example

```ts
import {
  Configuration,
  ShoppingListApi,
} from '';
import type { ClearCheckedRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new ShoppingListApi(config);

  try {
    const data = await api.clearChecked();
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

[**ClearCheckedResponse**](ClearCheckedResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Checked items cleared |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## createItems

> CreateShoppingListResponse createItems(createShoppingListRequest)



### Example

```ts
import {
  Configuration,
  ShoppingListApi,
} from '';
import type { CreateItemsRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new ShoppingListApi(config);

  const body = {
    // CreateShoppingListRequest
    createShoppingListRequest: ...,
  } satisfies CreateItemsRequest;

  try {
    const data = await api.createItems(body);
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
| **createShoppingListRequest** | [CreateShoppingListRequest](CreateShoppingListRequest.md) |  | |

### Return type

[**CreateShoppingListResponse**](CreateShoppingListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **201** | Items created |  -  |
| **400** | Invalid request |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## deleteItem

> deleteItem(id)



### Example

```ts
import {
  Configuration,
  ShoppingListApi,
} from '';
import type { DeleteItemRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new ShoppingListApi(config);

  const body = {
    // string | Shopping list item ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
  } satisfies DeleteItemRequest;

  try {
    const data = await api.deleteItem(body);
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
| **id** | `string` | Shopping list item ID | [Defaults to `undefined`] |

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
| **204** | Item deleted |  -  |
| **401** | Unauthorized |  -  |
| **404** | Item not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## listItems

> ShoppingListResponse listItems()



### Example

```ts
import {
  Configuration,
  ShoppingListApi,
} from '';
import type { ListItemsRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new ShoppingListApi(config);

  try {
    const data = await api.listItems();
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

[**ShoppingListResponse**](ShoppingListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | List of shopping list items |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## syncItems

> SyncResponse syncItems(syncRequest)



### Example

```ts
import {
  Configuration,
  ShoppingListApi,
} from '';
import type { SyncItemsRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new ShoppingListApi(config);

  const body = {
    // SyncRequest
    syncRequest: ...,
  } satisfies SyncItemsRequest;

  try {
    const data = await api.syncItems(body);
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
| **syncRequest** | [SyncRequest](SyncRequest.md) |  | |

### Return type

[**SyncResponse**](SyncResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Sync completed |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## updateItem

> updateItem(id, updateShoppingListItemRequest)



### Example

```ts
import {
  Configuration,
  ShoppingListApi,
} from '';
import type { UpdateItemRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new ShoppingListApi(config);

  const body = {
    // string | Shopping list item ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
    // UpdateShoppingListItemRequest
    updateShoppingListItemRequest: ...,
  } satisfies UpdateItemRequest;

  try {
    const data = await api.updateItem(body);
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
| **id** | `string` | Shopping list item ID | [Defaults to `undefined`] |
| **updateShoppingListItemRequest** | [UpdateShoppingListItemRequest](UpdateShoppingListItemRequest.md) |  | |

### Return type

`void` (Empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Item updated |  -  |
| **401** | Unauthorized |  -  |
| **404** | Item not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

