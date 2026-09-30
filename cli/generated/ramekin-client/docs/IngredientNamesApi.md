# \IngredientNamesApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**get_ingredient_names_status**](IngredientNamesApi.md#get_ingredient_names_status) | **GET** /api/ingredient-names/status | 
[**retry_ingredient_names**](IngredientNamesApi.md#retry_ingredient_names) | **POST** /api/ingredient-names/retry | 
[**warm_ingredient_names**](IngredientNamesApi.md#warm_ingredient_names) | **POST** /api/ingredient-names/warm | Queue every name the catalog doesn't know from the caller's current recipes and shopping list, e.g. once after this feature ships.



## get_ingredient_names_status

> models::IngredientNamesStatusResponse get_ingredient_names_status()


### Parameters

This endpoint does not need any parameter.

### Return type

[**models::IngredientNamesStatusResponse**](IngredientNamesStatusResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## retry_ingredient_names

> models::IngredientNamesQueuedResponse retry_ingredient_names()


### Parameters

This endpoint does not need any parameter.

### Return type

[**models::IngredientNamesQueuedResponse**](IngredientNamesQueuedResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## warm_ingredient_names

> models::IngredientNamesQueuedResponse warm_ingredient_names()
Queue every name the catalog doesn't know from the caller's current recipes and shopping list, e.g. once after this feature ships.

### Parameters

This endpoint does not need any parameter.

### Return type

[**models::IngredientNamesQueuedResponse**](IngredientNamesQueuedResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

