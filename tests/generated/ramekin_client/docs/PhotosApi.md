# ramekin_client.PhotosApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**upload**](PhotosApi.md#upload) | **POST** /api/photos | 


# **upload**
> UploadPhotoResponse upload()



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

    try:
        api_response = api_instance.upload()
        print("The response of PhotosApi->upload:\n")
        pprint(api_response)
    except Exception as e:
        print("Exception when calling PhotosApi->upload: %s\n" % e)
```



### Parameters

This endpoint does not need any parameter.

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

