# LookupImportJobsRequest

Request body for looking up import jobs by idempotency key

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**idempotency_keys** | **List[str]** | Keys previously sent as ImportRecipeRequest.idempotency_key (at most 1000) | 

## Example

```python
from ramekin_client.models.lookup_import_jobs_request import LookupImportJobsRequest

# TODO update the JSON string below
json = "{}"
# create an instance of LookupImportJobsRequest from a JSON string
lookup_import_jobs_request_instance = LookupImportJobsRequest.from_json(json)
# print the JSON string representation of the object
print(LookupImportJobsRequest.to_json())

# convert the object into a dict
lookup_import_jobs_request_dict = lookup_import_jobs_request_instance.to_dict()
# create an instance of LookupImportJobsRequest from a dict
lookup_import_jobs_request_from_dict = LookupImportJobsRequest.from_dict(lookup_import_jobs_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


