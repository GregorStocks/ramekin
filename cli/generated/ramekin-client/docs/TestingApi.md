# \TestingApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**ping**](TestingApi.md#ping) | **GET** /api/test/ping | 
[**unauthed_ping**](TestingApi.md#unauthed_ping) | **GET** /api/test/unauthed-ping | 



## ping

> models::PingResponse ping()


### Parameters

This endpoint does not need any parameter.

### Return type

[**models::PingResponse**](PingResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## unauthed_ping

> models::UnauthedPingResponse unauthed_ping()


### Parameters

This endpoint does not need any parameter.

### Return type

[**models::UnauthedPingResponse**](UnauthedPingResponse.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

