# PhotoSummary


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**content_type** | **str** |  | 
**created_at** | **datetime** |  | 
**id** | **str** |  | 
**thumbnail** | **str** | Base64-encoded JPEG thumbnail | 

## Example

```python
from ramekin_client.models.photo_summary import PhotoSummary

# TODO update the JSON string below
json = "{}"
# create an instance of PhotoSummary from a JSON string
photo_summary_instance = PhotoSummary.from_json(json)
# print the JSON string representation of the object
print(PhotoSummary.to_json())

# convert the object into a dict
photo_summary_dict = photo_summary_instance.to_dict()
# create an instance of PhotoSummary from a dict
photo_summary_from_dict = PhotoSummary.from_dict(photo_summary_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


