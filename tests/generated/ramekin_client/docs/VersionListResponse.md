# VersionListResponse

Response for version list endpoint

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**versions** | [**List[VersionSummary]**](VersionSummary.md) |  | 

## Example

```python
from ramekin_client.models.version_list_response import VersionListResponse

# TODO update the JSON string below
json = "{}"
# create an instance of VersionListResponse from a JSON string
version_list_response_instance = VersionListResponse.from_json(json)
# print the JSON string representation of the object
print(VersionListResponse.to_json())

# convert the object into a dict
version_list_response_dict = version_list_response_instance.to_dict()
# create an instance of VersionListResponse from a dict
version_list_response_from_dict = VersionListResponse.from_dict(version_list_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


