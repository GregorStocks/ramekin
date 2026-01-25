# EnrichmentInfo

Information about an enrichment type for the API.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**description** | **str** |  | 
**display_name** | **str** |  | 
**output_fields** | **List[str]** |  | 
**type** | **str** |  | 

## Example

```python
from ramekin_client.models.enrichment_info import EnrichmentInfo

# TODO update the JSON string below
json = "{}"
# create an instance of EnrichmentInfo from a JSON string
enrichment_info_instance = EnrichmentInfo.from_json(json)
# print the JSON string representation of the object
print(EnrichmentInfo.to_json())

# convert the object into a dict
enrichment_info_dict = enrichment_info_instance.to_dict()
# create an instance of EnrichmentInfo from a dict
enrichment_info_from_dict = EnrichmentInfo.from_dict(enrichment_info_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


