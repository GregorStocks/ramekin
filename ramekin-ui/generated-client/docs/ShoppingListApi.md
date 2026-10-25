# ShoppingListApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**listShoppingListHistory**](ShoppingListApi.md#listshoppinglisthistory) | **GET** /api/shopping-list/history |  |
| [**undoShoppingListChange**](ShoppingListApi.md#undoshoppinglistchange) | **POST** /api/shopping-list/history/{id}/undo |  |



## listShoppingListHistory

> ShoppingListHistoryResponse listShoppingListHistory(limit)



### Example

```ts
import {
  Configuration,
  ShoppingListApi,
} from '';
import type { ListShoppingListHistoryRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new ShoppingListApi(config);

  const body = {
    // number | Most entries to return, between 1 and 200. Defaults to 50. (optional)
    limit: 789,
  } satisfies ListShoppingListHistoryRequest;

  try {
    const data = await api.listShoppingListHistory(body);
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
| **limit** | `number` | Most entries to return, between 1 and 200. Defaults to 50. | [Optional] [Defaults to `undefined`] |

### Return type

[**ShoppingListHistoryResponse**](ShoppingListHistoryResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | The latest history entries |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## undoShoppingListChange

> ShoppingListHistoryEntry undoShoppingListChange(id)



### Example

```ts
import {
  Configuration,
  ShoppingListApi,
} from '';
import type { UndoShoppingListChangeRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new ShoppingListApi(config);

  const body = {
    // string | History entry ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
  } satisfies UndoShoppingListChangeRequest;

  try {
    const data = await api.undoShoppingListChange(body);
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
| **id** | `string` | History entry ID | [Defaults to `undefined`] |

### Return type

[**ShoppingListHistoryEntry**](ShoppingListHistoryEntry.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | Undone; the entry recording the undo. Sync to see it. |  -  |
| **401** | Unauthorized |  -  |
| **404** | No such entry |  -  |
| **409** | Already undone, or later changes replaced all of it |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

