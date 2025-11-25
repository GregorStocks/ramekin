# \PhotosApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**get_photo**](PhotosApi.md#get_photo) | **GET** /api/photos/{id} | 
[**list_photos**](PhotosApi.md#list_photos) | **GET** /api/photos | 
[**upload**](PhotosApi.md#upload) | **POST** /api/photos | 



## get_photo

> get_photo(id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Photo ID | [required] |

### Return type

 (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/octet-stream, application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## list_photos

> models::ListPhotosResponse list_photos()


### Parameters

This endpoint does not need any parameter.

### Return type

[**models::ListPhotosResponse**](ListPhotosResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## upload

> models::UploadPhotoResponse upload(file)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**file** | **std::path::PathBuf** |  | [required] |

### Return type

[**models::UploadPhotoResponse**](UploadPhotoResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: multipart/form-data
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

