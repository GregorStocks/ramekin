# ClientLogsAPI

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**createClientLog**](ClientLogsAPI.md#createclientlog) | **POST** /api/client-logs | 
[**getClientLog**](ClientLogsAPI.md#getclientlog) | **GET** /api/client-logs/{id} | 
[**listClientLogs**](ClientLogsAPI.md#listclientlogs) | **GET** /api/client-logs | 


# **createClientLog**
```swift
    open class func createClientLog(createClientLogRequest: CreateClientLogRequest, completion: @escaping (_ data: CreateClientLogResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let createClientLogRequest = CreateClientLogRequest(appVersion: "appVersion_example", content: "content_example", osInfo: "osInfo_example", platform: "platform_example") // CreateClientLogRequest | 

ClientLogsAPI.createClientLog(createClientLogRequest: createClientLogRequest) { (response, error) in
    guard error == nil else {
        print(error)
        return
    }

    if (response) {
        dump(response)
    }
}
```

### Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **createClientLogRequest** | [**CreateClientLogRequest**](CreateClientLogRequest.md) |  | 

### Return type

[**CreateClientLogResponse**](CreateClientLogResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **getClientLog**
```swift
    open class func getClientLog(id: UUID, completion: @escaping (_ data: GetClientLogResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Log upload id

ClientLogsAPI.getClientLog(id: id) { (response, error) in
    guard error == nil else {
        print(error)
        return
    }

    if (response) {
        dump(response)
    }
}
```

### Parameters

Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID** | Log upload id | 

### Return type

[**GetClientLogResponse**](GetClientLogResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **listClientLogs**
```swift
    open class func listClientLogs(completion: @escaping (_ data: ListClientLogsResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient


ClientLogsAPI.listClientLogs() { (response, error) in
    guard error == nil else {
        print(error)
        return
    }

    if (response) {
        dump(response)
    }
}
```

### Parameters
This endpoint does not need any parameter.

### Return type

[**ListClientLogsResponse**](ListClientLogsResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

