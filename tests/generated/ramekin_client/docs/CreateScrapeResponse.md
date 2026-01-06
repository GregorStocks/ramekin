# CreateScrapeResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **UUID** | The scrape job ID | 
**status** | **str** | Current job status | 

## Example

```python
from ramekin_client.models.create_scrape_response import CreateScrapeResponse

# TODO update the JSON string below
json = "{}"
# create an instance of CreateScrapeResponse from a JSON string
create_scrape_response_instance = CreateScrapeResponse.from_json(json)
# print the JSON string representation of the object
print(CreateScrapeResponse.to_json())

# convert the object into a dict
create_scrape_response_dict = create_scrape_response_instance.to_dict()
# create an instance of CreateScrapeResponse from a dict
create_scrape_response_from_dict = CreateScrapeResponse.from_dict(create_scrape_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


