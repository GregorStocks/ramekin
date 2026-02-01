# ScrapeJobResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**canRetry** | **Bool** | Whether this job can be retried | 
**error** | **String** | Error message if failed | [optional] 
**failedAtStep** | **String** | Which step failed (for retry logic) | [optional] 
**id** | **UUID** | The scrape job ID | 
**recipeId** | **UUID** | Recipe ID if completed successfully | [optional] 
**retryCount** | **Int** | Number of retry attempts | 
**status** | **String** | Current job status (pending, scraping, parsing, completed, failed) | 
**url** | **String** | URL being scraped (optional for imports) | [optional] 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


