# RenameTagRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**name** | **str** |  | 

## Example

```python
from ramekin_client.models.rename_tag_request import RenameTagRequest

# TODO update the JSON string below
json = "{}"
# create an instance of RenameTagRequest from a JSON string
rename_tag_request_instance = RenameTagRequest.from_json(json)
# print the JSON string representation of the object
print(RenameTagRequest.to_json())

# convert the object into a dict
rename_tag_request_dict = rename_tag_request_instance.to_dict()
# create an instance of RenameTagRequest from a dict
rename_tag_request_from_dict = RenameTagRequest.from_dict(rename_tag_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


