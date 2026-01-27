# RescrapeResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**job_id** | **UUID** | The scrape job ID | 
**status** | **str** | Current job status | 

## Example

```python
from ramekin_client.models.rescrape_response import RescrapeResponse

# TODO update the JSON string below
json = "{}"
# create an instance of RescrapeResponse from a JSON string
rescrape_response_instance = RescrapeResponse.from_json(json)
# print the JSON string representation of the object
print(RescrapeResponse.to_json())

# convert the object into a dict
rescrape_response_dict = rescrape_response_instance.to_dict()
# create an instance of RescrapeResponse from a dict
rescrape_response_from_dict = RescrapeResponse.from_dict(rescrape_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


