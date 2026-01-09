# \ScrapeApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**capture**](ScrapeApi.md#capture) | **POST** /api/scrape/capture | 
[**create_scrape**](ScrapeApi.md#create_scrape) | **POST** /api/scrape | 
[**get_scrape**](ScrapeApi.md#get_scrape) | **GET** /api/scrape/{id} | 
[**retry_scrape**](ScrapeApi.md#retry_scrape) | **POST** /api/scrape/{id}/retry | 



## capture

> models::CreateScrapeResponse capture(capture_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**capture_request** | [**CaptureRequest**](CaptureRequest.md) |  | [required] |

### Return type

[**models::CreateScrapeResponse**](CreateScrapeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## create_scrape

> models::CreateScrapeResponse create_scrape(create_scrape_request)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**create_scrape_request** | [**CreateScrapeRequest**](CreateScrapeRequest.md) |  | [required] |

### Return type

[**models::CreateScrapeResponse**](CreateScrapeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_scrape

> models::ScrapeJobResponse get_scrape(id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Scrape job ID | [required] |

### Return type

[**models::ScrapeJobResponse**](ScrapeJobResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## retry_scrape

> models::RetryScrapeResponse retry_scrape(id)


### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**id** | **uuid::Uuid** | Scrape job ID | [required] |

### Return type

[**models::RetryScrapeResponse**](RetryScrapeResponse.md)

### Authorization

[bearer_auth](../README.md#bearer_auth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)

