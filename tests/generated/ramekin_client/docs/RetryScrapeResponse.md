# RetryScrapeResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **UUID** | The scrape job ID | 
**status** | **str** | New job status after retry | 

## Example

```python
from ramekin_client.models.retry_scrape_response import RetryScrapeResponse

# TODO update the JSON string below
json = "{}"
# create an instance of RetryScrapeResponse from a JSON string
retry_scrape_response_instance = RetryScrapeResponse.from_json(json)
# print the JSON string representation of the object
print(RetryScrapeResponse.to_json())

# convert the object into a dict
retry_scrape_response_dict = retry_scrape_response_instance.to_dict()
# create an instance of RetryScrapeResponse from a dict
retry_scrape_response_from_dict = RetryScrapeResponse.from_dict(retry_scrape_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


