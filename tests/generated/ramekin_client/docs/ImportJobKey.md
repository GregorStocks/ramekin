# ImportJobKey

An import job submitted with an idempotency key

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**idempotency_key** | **str** |  | 
**job_id** | **UUID** |  | 

## Example

```python
from ramekin_client.models.import_job_key import ImportJobKey

# TODO update the JSON string below
json = "{}"
# create an instance of ImportJobKey from a JSON string
import_job_key_instance = ImportJobKey.from_json(json)
# print the JSON string representation of the object
print(ImportJobKey.to_json())

# convert the object into a dict
import_job_key_dict = import_job_key_instance.to_dict()
# create an instance of ImportJobKey from a dict
import_job_key_from_dict = ImportJobKey.from_dict(import_job_key_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


