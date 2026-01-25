# ListEnrichmentsResponse

Response from the list enrichments endpoint.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**enrichments** | [**List[EnrichmentInfo]**](EnrichmentInfo.md) | Available enrichment types. | 

## Example

```python
from ramekin_client.models.list_enrichments_response import ListEnrichmentsResponse

# TODO update the JSON string below
json = "{}"
# create an instance of ListEnrichmentsResponse from a JSON string
list_enrichments_response_instance = ListEnrichmentsResponse.from_json(json)
# print the JSON string representation of the object
print(ListEnrichmentsResponse.to_json())

# convert the object into a dict
list_enrichments_response_dict = list_enrichments_response_instance.to_dict()
# create an instance of ListEnrichmentsResponse from a dict
list_enrichments_response_from_dict = ListEnrichmentsResponse.from_dict(list_enrichments_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


