# ScrapeJobResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**can_retry** | **bool** | Whether this job can be retried | 
**created_at** | **datetime** | When the job was created | 
**error** | **str** | Error message if failed | [optional] 
**failed_at_step** | **str** | Which step failed (for retry logic) | [optional] 
**id** | **UUID** | The scrape job ID | 
**recipe_id** | **UUID** | Recipe ID once the recipe is saved (also set on a job that failed after saving; rescrapes have it from the start) | [optional] 
**retry_count** | **int** | Number of retry attempts | 
**status** | **str** | Current job status (pending, scraping, parsing, enriching, completed, failed). While \&quot;enriching\&quot; the recipe is saved and &#x60;recipe_id&#x60; is set; AI enrichment may still update it until the job completes. | 
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


