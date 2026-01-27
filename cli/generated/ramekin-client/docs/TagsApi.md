# \TagsApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_tag**](TagsApi.md#create_tag) | **POST** /api/tags | 
[**delete_tag**](TagsApi.md#delete_tag) | **DELETE** /api/tags/{id} | 
[**list_all_tags**](TagsApi.md#list_all_tags) | **GET** /api/tags | 



## create_tag

> models::CreateTagResponse create_tag(create_tag_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**create_tag_request** | [**CreateTagRequest**](CreateTagRequest.md) |  | [required] |

### Return type

[**models::CreateTagResponse**](CreateTagResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## delete_tag

> delete_tag(id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Tag ID | [required] |

### Return type

 (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## list_all_tags

> models::TagsListResponse list_all_tags()


### Parameters

This endpoint does not need any parameter.

### Return type

[**models::TagsListResponse**](TagsListResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

