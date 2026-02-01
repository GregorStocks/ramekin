# ScrapeAPI

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**capture**](ScrapeAPI.md#capture) | **POST** /api/scrape/capture | 
[**createScrape**](ScrapeAPI.md#createscrape) | **POST** /api/scrape | 
[**getScrape**](ScrapeAPI.md#getscrape) | **GET** /api/scrape/{id} | 
[**retryScrape**](ScrapeAPI.md#retryscrape) | **POST** /api/scrape/{id}/retry | 


# **capture**
```swift
    open class func capture(captureRequest: CaptureRequest, completion: @escaping (_ data: CreateScrapeResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let captureRequest = CaptureRequest(html: "html_example", sourceUrl: "sourceUrl_example") // CaptureRequest | 

ScrapeAPI.capture(captureRequest: captureRequest) { (response, error) in
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
 **captureRequest** | [**CaptureRequest**](CaptureRequest.md) |  | 

### Return type

[**CreateScrapeResponse**](CreateScrapeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **createScrape**
```swift
    open class func createScrape(createScrapeRequest: CreateScrapeRequest, completion: @escaping (_ data: CreateScrapeResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let createScrapeRequest = CreateScrapeRequest(url: "url_example") // CreateScrapeRequest | 

ScrapeAPI.createScrape(createScrapeRequest: createScrapeRequest) { (response, error) in
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
 **createScrapeRequest** | [**CreateScrapeRequest**](CreateScrapeRequest.md) |  | 

### Return type

[**CreateScrapeResponse**](CreateScrapeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: application/json
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **getScrape**
```swift
    open class func getScrape(id: UUID, completion: @escaping (_ data: ScrapeJobResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Scrape job ID

ScrapeAPI.getScrape(id: id) { (response, error) in
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
 **id** | **UUID** | Scrape job ID | 

### Return type

[**ScrapeJobResponse**](ScrapeJobResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **retryScrape**
```swift
    open class func retryScrape(id: UUID, completion: @escaping (_ data: RetryScrapeResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Scrape job ID

ScrapeAPI.retryScrape(id: id) { (response, error) in
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
 **id** | **UUID** | Scrape job ID | 

### Return type

[**RetryScrapeResponse**](RetryScrapeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

