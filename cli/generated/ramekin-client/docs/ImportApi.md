# \ImportApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**import_from_photos**](ImportApi.md#import_from_photos) | **POST** /api/import/photos | 
[**import_recipe**](ImportApi.md#import_recipe) | **POST** /api/import/recipe | 
[**prepare_text_recipe**](ImportApi.md#prepare_text_recipe) | **POST** /api/import/text | 



## import_from_photos

> models::ImportFromPhotosResponse import_from_photos(import_from_photos_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**import_from_photos_request** | [**ImportFromPhotosRequest**](ImportFromPhotosRequest.md) |  | [required] |

### Return type

[**models::ImportFromPhotosResponse**](ImportFromPhotosResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## import_recipe

> models::ImportRecipeResponse import_recipe(import_recipe_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**import_recipe_request** | [**ImportRecipeRequest**](ImportRecipeRequest.md) |  | [required] |

### Return type

[**models::ImportRecipeResponse**](ImportRecipeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## prepare_text_recipe

> models::PrepareTextRecipeResponse prepare_text_recipe(prepare_text_recipe_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**prepare_text_recipe_request** | [**PrepareTextRecipeRequest**](PrepareTextRecipeRequest.md) |  | [required] |

### Return type

[**models::PrepareTextRecipeResponse**](PrepareTextRecipeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

