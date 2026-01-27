# TagsListResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**tags** | [**List[TagItem]**](TagItem.md) |  | 

## Example

```python
from ramekin_client.models.tags_list_response import TagsListResponse

# TODO update the JSON string below
json = "{}"
# create an instance of TagsListResponse from a JSON string
tags_list_response_instance = TagsListResponse.from_json(json)
# print the JSON string representation of the object
print(TagsListResponse.to_json())

# convert the object into a dict
tags_list_response_dict = tags_list_response_instance.to_dict()
# create an instance of TagsListResponse from a dict
tags_list_response_from_dict = TagsListResponse.from_dict(tags_list_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


