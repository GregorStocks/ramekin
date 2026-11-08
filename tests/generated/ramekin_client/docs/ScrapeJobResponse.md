# ScrapeJobResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**can_retry** | **bool** | Whether &#x60;POST /api/scrape/{id}/retry&#x60; would accept this job: it failed, has retries left, and has a source to start again from. | 
**created_at** | **datetime** | When the job was created | 
**error** | **str** | Error message if failed | [optional] 
**failed_at_step** | **str** | Which step failed (for retry logic) | [optional] 
**id** | **UUID** | The scrape job ID | 
**is_terminal** | **bool** | Whether the job has stopped (&#x60;completed&#x60; or &#x60;failed&#x60;): stop polling. | 
**recipe_id** | **UUID** | Recipe ID once the recipe is saved (also set on a job that failed after saving; rescrapes and normalize jobs have it from the start) | [optional] 
**recipe_ready** | **bool** | Whether &#x60;recipe_id&#x60; names a recipe the client can open now: the job has saved it (even if a later normalize step then failed) or finished working on it. Normalize ops may still update it while the job is &#x60;normalizing&#x60;. | 
**retry_count** | **int** | Number of retry attempts | 
**status** | [**ScrapeJobStatus**](ScrapeJobStatus.md) | Current job status. While an import or scrape is \&quot;normalizing\&quot; the recipe is saved and &#x60;recipe_id&#x60; is set; normalize ops may still update it until the job completes. A manual normalize job saves nothing: it ends with a proposal (see &#x60;GET /api/recipes/{id}/normalize/{job_id}&#x60;). | 
**steps** | [**List[StepState]**](StepState.md) | Per-step state for the status page (ordered by pipeline step). | 
**url** | **str** | URL being scraped (optional for imports) | [optional] 

## Example

```python
from ramekin_client.models.scrape_job_response import ScrapeJobResponse

# TODO update the JSON string below
json = "{}"
# create an instance of ScrapeJobResponse from a JSON string
scrape_job_response_instance = ScrapeJobResponse.from_json(json)
# print the JSON string representation of the object
print(ScrapeJobResponse.to_json())

# convert the object into a dict
scrape_job_response_dict = scrape_job_response_instance.to_dict()
# create an instance of ScrapeJobResponse from a dict
scrape_job_response_from_dict = ScrapeJobResponse.from_dict(scrape_job_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


