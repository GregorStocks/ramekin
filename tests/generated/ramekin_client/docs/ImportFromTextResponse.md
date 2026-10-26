# ImportFromTextResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**job_id** | **UUID** | The created job ID | 
**status** | **str** | Current job status | 

## Example

```python
from ramekin_client.models.import_from_text_response import ImportFromTextResponse

# TODO update the JSON string below
json = "{}"
# create an instance of ImportFromTextResponse from a JSON string
import_from_text_response_instance = ImportFromTextResponse.from_json(json)
# print the JSON string representation of the object
print(ImportFromTextResponse.to_json())

# convert the object into a dict
import_from_text_response_dict = import_from_text_response_instance.to_dict()
# create an instance of ImportFromTextResponse from a dict
import_from_text_response_from_dict = ImportFromTextResponse.from_dict(import_from_text_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


