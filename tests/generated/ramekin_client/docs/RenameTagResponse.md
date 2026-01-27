# RenameTagResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **UUID** |  | 
**name** | **str** |  | 

## Example

```python
from ramekin_client.models.rename_tag_response import RenameTagResponse

# TODO update the JSON string below
json = "{}"
# create an instance of RenameTagResponse from a JSON string
rename_tag_response_instance = RenameTagResponse.from_json(json)
# print the JSON string representation of the object
print(RenameTagResponse.to_json())

# convert the object into a dict
rename_tag_response_dict = rename_tag_response_instance.to_dict()
# create an instance of RenameTagResponse from a dict
rename_tag_response_from_dict = RenameTagResponse.from_dict(rename_tag_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


