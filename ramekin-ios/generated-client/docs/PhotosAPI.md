# PhotosAPI

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**getPhoto**](PhotosAPI.md#getphoto) | **GET** /api/photos/{id} | 
[**getPhotoThumbnail**](PhotosAPI.md#getphotothumbnail) | **GET** /api/photos/{id}/thumbnail | 
[**upload**](PhotosAPI.md#upload) | **POST** /api/photos | 


# **getPhoto**
```swift
    open class func getPhoto(id: UUID, completion: @escaping (_ data: Void?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Photo ID

PhotosAPI.getPhoto(id: id) { (response, error) in
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
 **id** | **UUID** | Photo ID | 

### Return type

Void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/octet-stream, application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **getPhotoThumbnail**
```swift
    open class func getPhotoThumbnail(id: UUID, size: Int? = nil, completion: @escaping (_ data: Void?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let id = 987 // UUID | Photo ID
let size = 987 // Int | Desired thumbnail size in pixels (longest edge). Clamped to 1..=800. Default: 200. (optional)

PhotosAPI.getPhotoThumbnail(id: id, size: size) { (response, error) in
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
 **id** | **UUID** | Photo ID | 
 **size** | **Int** | Desired thumbnail size in pixels (longest edge). Clamped to 1..&#x3D;800. Default: 200. | [optional] 

### Return type

Void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: image/jpeg, application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **upload**
```swift
    open class func upload(file: URL, completion: @escaping (_ data: UploadPhotoResponse?, _ error: Error?) -> Void)
```



### Example
```swift
// The following code samples are still beta. For any issue, please report via http://github.com/OpenAPITools/openapi-generator/issues/new
import RamekinClient

let file = URL(string: "https://example.com")! // URL | 

PhotosAPI.upload(file: file) { (response, error) in
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
 **file** | **URL** |  | 

### Return type

[**UploadPhotoResponse**](UploadPhotoResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: multipart/form-data
 - **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

