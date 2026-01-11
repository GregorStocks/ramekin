# VersionSummary

Version summary for listing version history

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**created_at** | **datetime** |  | 
**id** | **UUID** |  | 
**is_current** | **bool** |  | 
**title** | **str** |  | 
**version_source** | **str** |  | 

## Example

```python
from ramekin_client.models.version_summary import VersionSummary

# TODO update the JSON string below
json = "{}"
# create an instance of VersionSummary from a JSON string
version_summary_instance = VersionSummary.from_json(json)
# print the JSON string representation of the object
print(VersionSummary.to_json())

# convert the object into a dict
version_summary_dict = version_summary_instance.to_dict()
# create an instance of VersionSummary from a dict
version_summary_from_dict = VersionSummary.from_dict(version_summary_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


