# \ClientLogsApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_client_log**](ClientLogsApi.md#create_client_log) | **POST** /api/client-logs | 
[**get_client_log**](ClientLogsApi.md#get_client_log) | **GET** /api/client-logs/{id} | 
[**list_client_logs**](ClientLogsApi.md#list_client_logs) | **GET** /api/client-logs | 



## create_client_log

> models::CreateClientLogResponse create_client_log(create_client_log_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**create_client_log_request** | [**CreateClientLogRequest**](CreateClientLogRequest.md) |  | [required] |

### Return type

[**models::CreateClientLogResponse**](CreateClientLogResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_client_log

> models::GetClientLogResponse get_client_log(id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Log upload id | [required] |

### Return type

[**models::GetClientLogResponse**](GetClientLogResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## list_client_logs

> models::ListClientLogsResponse list_client_logs()


### Parameters

This endpoint does not need any parameter.

### Return type

[**models::ListClientLogsResponse**](ListClientLogsResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

