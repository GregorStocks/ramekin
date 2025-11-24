# \AuthApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**hello**](AuthApi.md#hello) | **GET** /api/auth/hello | 
[**login**](AuthApi.md#login) | **POST** /api/auth/login | 
[**signup**](AuthApi.md#signup) | **POST** /api/auth/signup | 



## hello

> models::HelloResponse hello()


### Parameters

This endpoint does not need any parameter.

### Return type

[**models::HelloResponse**](HelloResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## login

> models::LoginResponse login(login_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**login_request** | [**LoginRequest**](LoginRequest.md) |  | [required] |

### Return type

[**models::LoginResponse**](LoginResponse.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## signup

> models::SignupResponse signup(signup_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**signup_request** | [**SignupRequest**](SignupRequest.md) |  | [required] |

### Return type

[**models::SignupResponse**](SignupResponse.md)

### Authorization

No authorization required

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

