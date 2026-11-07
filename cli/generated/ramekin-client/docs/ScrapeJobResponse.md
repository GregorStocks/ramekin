# ScrapeJobResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**can_retry** | **bool** | Whether `POST /api/scrape/{id}/retry` would accept this job: it failed, has retries left, and has a source to start again from. | 
**created_at** | **String** | When the job was created | 
**error** | Option<**String**> | Error message if failed | [optional]
**failed_at_step** | Option<**String**> | Which step failed (for retry logic) | [optional]
**id** | [**uuid::Uuid**](uuid::Uuid.md) | The scrape job ID | 
**is_terminal** | **bool** | Whether the job has stopped (`completed` or `failed`): stop polling. | 
**recipe_id** | Option<[**uuid::Uuid**](uuid::Uuid.md)> | Recipe ID once the recipe is saved (also set on a job that failed after saving; rescrapes and normalize jobs have it from the start) | [optional]
**recipe_ready** | **bool** | Whether `recipe_id` names a recipe the client can open now: the job has saved it (even if a later normalize step then failed) or finished working on it. Normalize ops may still update it while the job is `normalizing`. | 
**retry_count** | **i32** | Number of retry attempts | 
**status** | [**models::ScrapeJobStatus**](ScrapeJobStatus.md) | Current job status. While an import or scrape is \"normalizing\" the recipe is saved and `recipe_id` is set; normalize ops may still update it until the job completes. A manual normalize job saves nothing: it ends with a proposal (see `GET /api/recipes/{id}/normalize/{job_id}`). | 
**steps** | [**Vec<models::StepState>**](StepState.md) | Per-step state for the status page (ordered by pipeline step). | 
**url** | Option<**String**> | URL being scraped (optional for imports) | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


