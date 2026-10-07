# LookupImportJobsResponse

The caller's import jobs for the requested keys; unknown keys are omitted

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**jobs** | [**List[ImportJobKey]**](ImportJobKey.md) |  | 

## Example

```python
from ramekin_client.models.lookup_import_jobs_response import LookupImportJobsResponse

# TODO update the JSON string below
json = "{}"
# create an instance of LookupImportJobsResponse from a JSON string
lookup_import_jobs_response_instance = LookupImportJobsResponse.from_json(json)
# print the JSON string representation of the object
print(LookupImportJobsResponse.to_json())

# convert the object into a dict
lookup_import_jobs_response_dict = lookup_import_jobs_response_instance.to_dict()
# create an instance of LookupImportJobsResponse from a dict
lookup_import_jobs_response_from_dict = LookupImportJobsResponse.from_dict(lookup_import_jobs_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


