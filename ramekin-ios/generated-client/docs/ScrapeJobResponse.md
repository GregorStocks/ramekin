# ScrapeJobResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**canRetry** | **Bool** | Whether this job can be retried | 
**createdAt** | **Date** | When the job was created | 
**error** | **String** | Error message if failed | [optional] 
**failedAtStep** | **String** | Which step failed (for retry logic) | [optional] 
**id** | **UUID** | The scrape job ID | 
**recipeId** | **UUID** | Recipe ID once the recipe is saved (status enriching or completed; rescrapes have it from the start) | [optional] 
**retryCount** | **Int** | Number of retry attempts | 
**status** | **String** | Current job status (pending, scraping, parsing, enriching, completed, failed). While \&quot;enriching\&quot; the recipe is saved and &#x60;recipe_id&#x60; is set; AI enrichment may still update it until the job completes. | 
**steps** | [StepState] | Per-step state for the status page (ordered by pipeline step). | 
**url** | **String** | URL being scraped (optional for imports) | [optional] 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


