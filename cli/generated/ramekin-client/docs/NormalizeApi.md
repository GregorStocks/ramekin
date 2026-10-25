# \NormalizeApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**accept_normalize_proposal**](NormalizeApi.md#accept_normalize_proposal) | **POST** /api/recipes/{id}/normalize/{job_id}/accept | 
[**get_normalize_proposal**](NormalizeApi.md#get_normalize_proposal) | **GET** /api/recipes/{id}/normalize/{job_id} | 
[**list_normalize_ops**](NormalizeApi.md#list_normalize_ops) | **GET** /api/recipes/normalize-ops | 
[**normalize_recipe**](NormalizeApi.md#normalize_recipe) | **POST** /api/recipes/{id}/normalize | 
[**normalize_recipes**](NormalizeApi.md#normalize_recipes) | **POST** /api/recipes/normalize | 



## accept_normalize_proposal

> models::AcceptNormalizeResponse accept_normalize_proposal(id, job_id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Recipe ID | [required] |
**job_id** | **uuid::Uuid** | Normalize job ID | [required] |

### Return type

[**models::AcceptNormalizeResponse**](AcceptNormalizeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_normalize_proposal

> models::NormalizeProposalResponse get_normalize_proposal(id, job_id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Recipe ID | [required] |
**job_id** | **uuid::Uuid** | Normalize job ID | [required] |

### Return type

[**models::NormalizeProposalResponse**](NormalizeProposalResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## list_normalize_ops

> models::NormalizeOpsResponse list_normalize_ops(recipe_id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**recipe_id** | Option<**uuid::Uuid**> | Report which ops can run on this recipe. |  |

### Return type

[**models::NormalizeOpsResponse**](NormalizeOpsResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## normalize_recipe

> models::NormalizeJobResponse normalize_recipe(id, normalize_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Recipe ID | [required] |
**normalize_request** | [**NormalizeRequest**](NormalizeRequest.md) |  | [required] |

### Return type

[**models::NormalizeJobResponse**](NormalizeJobResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## normalize_recipes

> models::BulkNormalizeResponse normalize_recipes(bulk_normalize_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**bulk_normalize_request** | [**BulkNormalizeRequest**](BulkNormalizeRequest.md) |  | [required] |

### Return type

[**models::BulkNormalizeResponse**](BulkNormalizeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

