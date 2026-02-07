# ramekin_client.PhotosApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**get_photo**](PhotosApi.md#get_photo) | **GET** /api/photos/{id} | 
[**get_photo_thumbnail**](PhotosApi.md#get_photo_thumbnail) | **GET** /api/photos/{id}/thumbnail | 
[**upload**](PhotosApi.md#upload) | **POST** /api/photos | 


# **get_photo**
> get_photo(id)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.rest import ApiException
from pprint import pprint

# Defining the host is optional and defaults to http://localhost
# See configuration.py for a list of all supported configuration parameters.
configuration = ramekin_client.Configuration(
    host = "http://localhost"
)

# The client must configure the authentication and authorization parameters
# in accordance with the API server security policy.
# Examples for each auth method are provided below, use the example that
# satisfies your auth use case.

# Configure Bearer authorization: bearer_auth
configuration = ramekin_client.Configuration(
    access_token = os.environ["BEARER_TOKEN"]
)

# Enter a context with an instance of the API client
with ramekin_client.ApiClient(configuration) as api_client:
    # Create an instance of the API class
    api_instance = ramekin_client.PhotosApi(api_client)
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Photo ID

    try:
        api_instance.get_photo(id)
    except Exception as e:
        print("Exception when calling PhotosApi->get_photo: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Photo ID | 

### Return type

void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: application/octet-stream, application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Photo data |  -  |
**401** | Unauthorized |  -  |
**404** | Photo not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **get_photo_thumbnail**
> get_photo_thumbnail(id, size=size)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.rest import ApiException
from pprint import pprint

# Defining the host is optional and defaults to http://localhost
# See configuration.py for a list of all supported configuration parameters.
configuration = ramekin_client.Configuration(
    host = "http://localhost"
)

# The client must configure the authentication and authorization parameters
# in accordance with the API server security policy.
# Examples for each auth method are provided below, use the example that
# satisfies your auth use case.

# Configure Bearer authorization: bearer_auth
configuration = ramekin_client.Configuration(
    access_token = os.environ["BEARER_TOKEN"]
)

# Enter a context with an instance of the API client
with ramekin_client.ApiClient(configuration) as api_client:
    # Create an instance of the API class
    api_instance = ramekin_client.PhotosApi(api_client)
    id = UUID('38400000-8cf0-11bd-b23e-10b96e4ef00d') # UUID | Photo ID
    size = 56 # int | Desired thumbnail size in pixels (longest edge). Clamped to 1..=800. Default: 200. (optional)

    try:
        api_instance.get_photo_thumbnail(id, size=size)
    except Exception as e:
        print("Exception when calling PhotosApi->get_photo_thumbnail: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **id** | **UUID**| Photo ID | 
 **size** | **int**| Desired thumbnail size in pixels (longest edge). Clamped to 1..&#x3D;800. Default: 200. | [optional] 

### Return type

void (empty response body)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: Not defined
 - **Accept**: image/jpeg, application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**200** | Photo thumbnail data |  -  |
**401** | Unauthorized |  -  |
**404** | Photo not found |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

# **upload**
> UploadPhotoResponse upload(file)

### Example

* Bearer Authentication (bearer_auth):

```python
import ramekin_client
from ramekin_client.models.upload_photo_response import UploadPhotoResponse
from ramekin_client.rest import ApiException
from pprint import pprint

# Defining the host is optional and defaults to http://localhost
# See configuration.py for a list of all supported configuration parameters.
configuration = ramekin_client.Configuration(
    host = "http://localhost"
)

# The client must configure the authentication and authorization parameters
# in accordance with the API server security policy.
# Examples for each auth method are provided below, use the example that
# satisfies your auth use case.

# Configure Bearer authorization: bearer_auth
configuration = ramekin_client.Configuration(
    access_token = os.environ["BEARER_TOKEN"]
)

# Enter a context with an instance of the API client
with ramekin_client.ApiClient(configuration) as api_client:
    # Create an instance of the API class
    api_instance = ramekin_client.PhotosApi(api_client)
    file = None # bytearray | 

    try:
        api_response = api_instance.upload(file)
        print("The response of PhotosApi->upload:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling PhotosApi->upload: %s\n" % e)
```



### Parameters


Name | Type | Description  | Notes
------------- | ------------- | ------------- | -------------
 **file** | **bytearray**|  | 

### Return type

[**UploadPhotoResponse**](UploadPhotoResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

 - **Content-Type**: multipart/form-data
 - **Accept**: application/json

### HTTP response details

| Status code | Description | Response headers |
|-------------|-------------|------------------|
**201** | Photo uploaded successfully |  -  |
**400** | Invalid request |  -  |
**401** | Unauthorized |  -  |

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

