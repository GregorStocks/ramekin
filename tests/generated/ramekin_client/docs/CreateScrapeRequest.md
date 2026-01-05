# CreateScrapeRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**url** | **str** | URL to scrape for recipe data | 

## Example

```python
from ramekin_client.models.create_scrape_request import CreateScrapeRequest

# TODO update the JSON string below
json = "{}"
# create an instance of CreateScrapeRequest from a JSON string
create_scrape_request_instance = CreateScrapeRequest.from_json(json)
# print the JSON string representation of the object
print(CreateScrapeRequest.to_json())

# convert the object into a dict
create_scrape_request_dict = create_scrape_request_instance.to_dict()
# create an instance of CreateScrapeRequest from a dict
create_scrape_request_from_dict = CreateScrapeRequest.from_dict(create_scrape_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


