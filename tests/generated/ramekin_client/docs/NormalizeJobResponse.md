# NormalizeJobResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**job_id** | **UUID** | The job; poll it with &#x60;GET /api/scrape/{id}&#x60;. | 
**status** | **str** |  | 

## Example

```python
from ramekin_client.models.normalize_job_response import NormalizeJobResponse

# TODO update the JSON string below
json = "{}"
# create an instance of NormalizeJobResponse from a JSON string
normalize_job_response_instance = NormalizeJobResponse.from_json(json)
# print the JSON string representation of the object
print(NormalizeJobResponse.to_json())

# convert the object into a dict
normalize_job_response_dict = normalize_job_response_instance.to_dict()
# create an instance of NormalizeJobResponse from a dict
normalize_job_response_from_dict = NormalizeJobResponse.from_dict(normalize_job_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


