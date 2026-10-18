# MealPlansApi

All URIs are relative to *http://localhost*

| Method | HTTP request | Description |
|------------- | ------------- | -------------|
| [**listMealPlans**](MealPlansApi.md#listmealplans) | **GET** /api/meal-plans |  |



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

