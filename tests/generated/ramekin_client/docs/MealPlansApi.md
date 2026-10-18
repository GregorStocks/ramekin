# ramekin_client.MealPlansApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**list_meal_plans**](MealPlansApi.md#list_meal_plans) | **GET** /api/meal-plans | 


# **list_meal_plans**
> MealPlanListResponse list_meal_plans(start_date=start_date, end_date=end_date)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.meal_plan_list_response import MealPlanListResponse
from ramekin_client.rest import ApiException
from pprint import pprint

# Defining the host is optional and defaults to http://localhost
# See configuration.py for a list of all supported configuration parameters.
configuration = ramekin_client.Configuration(
    host = "http://localhost"
)

# The client must configure the authentication and authorization parameters
# in accordance with the API server security policy.
# Examples for each auth method are provided below, use the example that
# satisfies your auth use case.

# Configure Bearer authorization: bearer_auth
configuration = ramekin_client.Configuration(
    access_token = os.environ["BEARER_TOKEN"]
)

# Enter a context with an instance of the API client
with ramekin_client.ApiClient(configuration) as api_client:
    # Create an instance of the API class
    api_instance = ramekin_client.MealPlansApi(api_client)
    start_date = '2013-10-20' # date | Start date (inclusive), format: YYYY-MM-DD. Defaults to today. (optional)
    end_date = '2013-10-20' # date | End date (inclusive), format: YYYY-MM-DD. Defaults to start_date + 6 days (one week). (optional)

    try:
        api_response = api_instance.list_meal_plans(start_date=start_date, end_date=end_date)
        print("The response of MealPlansApi->list_meal_plans:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling MealPlansApi->list_meal_plans: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **start_date** | **date**| Start date (inclusive), format: YYYY-MM-DD. Defaults to today. | [optional] 
 **end_date** | **date**| End date (inclusive), format: YYYY-MM-DD. Defaults to start_date + 6 days (one week). | [optional] 

### Return type

[**MealPlanListResponse**](MealPlanListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | List of meal plans for date range |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

