# MealPlansApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**createMealPlan**](MealPlansApi.md#createmealplanoperation) | **POST** /api/meal-plans |  |
| [**deleteMealPlan**](MealPlansApi.md#deletemealplan) | **DELETE** /api/meal-plans/{id} |  |
| [**listMealPlans**](MealPlansApi.md#listmealplans) | **GET** /api/meal-plans |  |
| [**updateMealPlan**](MealPlansApi.md#updatemealplanoperation) | **PUT** /api/meal-plans/{id} |  |



## createMealPlan

> CreateMealPlanResponse createMealPlan(createMealPlanRequest)



### Example

```ts
import {
  Configuration,
  MealPlansApi,
} from '';
import type { CreateMealPlanOperationRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new MealPlansApi(config);

  const body = {
    // CreateMealPlanRequest
    createMealPlanRequest: ...,
  } satisfies CreateMealPlanOperationRequest;

  try {
    const data = await api.createMealPlan(body);
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
| **createMealPlanRequest** | [CreateMealPlanRequest](CreateMealPlanRequest.md) |  | |

### Return type

[**CreateMealPlanResponse**](CreateMealPlanResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: `application/json`
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **201** | Meal plan created |  -  |
| **400** | Invalid request (recipe not found or deleted) |  -  |
| **401** | Unauthorized |  -  |
| **409** | Duplicate meal plan entry |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## deleteMealPlan

> deleteMealPlan(id)



### Example

```ts
import {
  Configuration,
  MealPlansApi,
} from '';
import type { DeleteMealPlanRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new MealPlansApi(config);

  const body = {
    // string | Meal plan ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
  } satisfies DeleteMealPlanRequest;

  try {
    const data = await api.deleteMealPlan(body);
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
| **id** | `string` | Meal plan ID | [Defaults to `undefined`] |

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
| **204** | Meal plan deleted |  -  |
| **401** | Unauthorized |  -  |
| **404** | Meal plan not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## listMealPlans

> MealPlanListResponse listMealPlans(startDate, endDate)



### Example

```ts
import {
  Configuration,
  MealPlansApi,
} from '';
import type { ListMealPlansRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new MealPlansApi(config);

  const body = {
    // Date | Start date (inclusive), format: YYYY-MM-DD. Defaults to today. (optional)
    startDate: 2013-10-20,
    // Date | End date (inclusive), format: YYYY-MM-DD. Defaults to start_date + 6 days (one week). (optional)
    endDate: 2013-10-20,
  } satisfies ListMealPlansRequest;

  try {
    const data = await api.listMealPlans(body);
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
| **startDate** | `Date` | Start date (inclusive), format: YYYY-MM-DD. Defaults to today. | [Optional] [Defaults to `undefined`] |
| **endDate** | `Date` | End date (inclusive), format: YYYY-MM-DD. Defaults to start_date + 6 days (one week). | [Optional] [Defaults to `undefined`] |

### Return type

[**MealPlanListResponse**](MealPlanListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: `application/json`


### HTTP response details
| Status code | Description | Response headers |
|-------------|-------------|------------------|
| **200** | List of meal plans for date range |  -  |
| **401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


## updateMealPlan

> updateMealPlan(id, updateMealPlanRequest)



### Example

```ts
import {
  Configuration,
  MealPlansApi,
} from '';
import type { UpdateMealPlanOperationRequest } from '';

async function example() {
  console.log("🚀 Testing  SDK...");
  const config = new Configuration({ 
    // Configure HTTP bearer authorization: bearer_auth
    accessToken: "YOUR BEARER TOKEN",
  });
  const api = new MealPlansApi(config);

  const body = {
    // string | Meal plan ID
    id: 38400000-8cf0-11bd-b23e-10b96e4ef00d,
    // UpdateMealPlanRequest
    updateMealPlanRequest: ...,
  } satisfies UpdateMealPlanOperationRequest;

  try {
    const data = await api.updateMealPlan(body);
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
| **id** | `string` | Meal plan ID | [Defaults to `undefined`] |
| **updateMealPlanRequest** | [UpdateMealPlanRequest](UpdateMealPlanRequest.md) |  | |

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
| **200** | Meal plan updated |  -  |
| **401** | Unauthorized |  -  |
| **404** | Meal plan not found |  -  |
| **409** | Conflict with existing meal plan |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)

