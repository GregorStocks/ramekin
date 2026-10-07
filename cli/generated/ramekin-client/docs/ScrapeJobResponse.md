# ScrapeJobResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**can_retry** | **bool** | Whether this job can be retried | 
**created_at** | **String** | When the job was created | 
**error** | Option<**String**> | Error message if failed | [optional]
**failed_at_step** | Option<**String**> | Which step failed (for retry logic) | [optional]
**id** | [**uuid::Uuid**](uuid::Uuid.md) | The scrape job ID | 
**recipe_id** | Option<[**uuid::Uuid**](uuid::Uuid.md)> | Recipe ID once the recipe is saved (also set on a job that failed after saving; rescrapes have it from the start) | [optional]
**retry_count** | **i32** | Number of retry attempts | 
**status** | **String** | Current job status (pending, scraping, parsing, enriching, completed, failed). While \"enriching\" the recipe is saved and `recipe_id` is set; AI enrichment may still update it until the job completes. | 
**steps** | [**Vec<models::StepState>**](StepState.md) | Per-step state for the status page (ordered by pipeline step). | 
**url** | Option<**String**> | URL being scraped (optional for imports) | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


