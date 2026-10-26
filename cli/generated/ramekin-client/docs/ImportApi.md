# \ImportApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**import_from_photos**](ImportApi.md#import_from_photos) | **POST** /api/import/photos | 
[**import_from_text**](ImportApi.md#import_from_text) | **POST** /api/import/text | 
[**import_recipe**](ImportApi.md#import_recipe) | **POST** /api/import/recipe | 
[**lookup_import_jobs**](ImportApi.md#lookup_import_jobs) | **POST** /api/import/recipe/lookup | 



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


## import_from_text

> models::ImportFromTextResponse import_from_text(import_from_text_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**import_from_text_request** | [**ImportFromTextRequest**](ImportFromTextRequest.md) |  | [required] |

### Return type

[**models::ImportFromTextResponse**](ImportFromTextResponse.md)

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


## lookup_import_jobs

> models::LookupImportJobsResponse lookup_import_jobs(lookup_import_jobs_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**lookup_import_jobs_request** | [**LookupImportJobsRequest**](LookupImportJobsRequest.md) |  | [required] |

### Return type

[**models::LookupImportJobsResponse**](LookupImportJobsResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

