# ImportFromTextRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**text** | **str** | A whole recipe, typed or pasted as unstructured text. | 

## Example

```python
from ramekin_client.models.import_from_text_request import ImportFromTextRequest

# TODO update the JSON string below
json = "{}"
# create an instance of ImportFromTextRequest from a JSON string
import_from_text_request_instance = ImportFromTextRequest.from_json(json)
# print the JSON string representation of the object
print(ImportFromTextRequest.to_json())

# convert the object into a dict
import_from_text_request_dict = import_from_text_request_instance.to_dict()
# create an instance of ImportFromTextRequest from a dict
import_from_text_request_from_dict = ImportFromTextRequest.from_dict(import_from_text_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


