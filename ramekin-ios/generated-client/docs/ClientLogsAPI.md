# ClientLogsAPI

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**createClientLog**](ClientLogsAPI.md#createclientlog) | **POST** /api/client-logs | 


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

