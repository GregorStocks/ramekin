# ScrapeJobResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**canRetry** | **Bool** | Whether &#x60;POST /api/scrape/{id}/retry&#x60; would accept this job: it failed, has retries left, and has a source to start again from. | 
**createdAt** | **Date** | When the job was created | 
**error** | **String** | Error message if failed | [optional] 
**failedAtStep** | **String** | Which step failed (for retry logic) | [optional] 
**id** | **UUID** | The scrape job ID | 
**isTerminal** | **Bool** | Whether the job has stopped (&#x60;completed&#x60; or &#x60;failed&#x60;): stop polling. | 
**recipeId** | **UUID** | Recipe ID once the recipe is saved (also set on a job that failed after saving; rescrapes and normalize jobs have it from the start) | [optional] 
**recipeReady** | **Bool** | Whether &#x60;recipe_id&#x60; names a recipe the client can open now: the job has saved it (even if a later normalize step then failed) or finished working on it. Normalize ops may still update it while the job is &#x60;normalizing&#x60;. | 
**retryCount** | **Int** | Number of retry attempts | 
**status** | [**ScrapeJobStatus**](ScrapeJobStatus.md) | Current job status. While an import or scrape is \&quot;normalizing\&quot; the recipe is saved and &#x60;recipe_id&#x60; is set; normalize ops may still update it until the job completes. A manual normalize job saves nothing: it ends with a proposal (see &#x60;GET /api/recipes/{id}/normalize/{job_id}&#x60;). | 
**steps** | [StepState] | Per-step state for the status page (ordered by pipeline step). | 
**url** | **String** | URL being scraped (optional for imports) | [optional] 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


