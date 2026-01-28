# ScrapeJobResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**can_retry** | **bool** | Whether this job can be retried | 
**error** | Option<**String**> | Error message if failed | [optional]
**failed_at_step** | Option<**String**> | Which step failed (for retry logic) | [optional]
**id** | [**uuid::Uuid**](uuid::Uuid.md) | The scrape job ID | 
**recipe_id** | Option<[**uuid::Uuid**](uuid::Uuid.md)> | Recipe ID if completed successfully | [optional]
**retry_count** | **i32** | Number of retry attempts | 
**status** | **String** | Current job status (pending, scraping, parsing, completed, failed) | 
**url** | Option<**String**> | URL being scraped (optional for imports) | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


