# CustomEnrichRequest

Request body for custom enrichment.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**instruction** | **str** |  | 
**recipe** | [**RecipeContent**](RecipeContent.md) |  | 

## Example

```python
from ramekin_client.models.custom_enrich_request import CustomEnrichRequest

# TODO update the JSON string below
json = "{}"
# create an instance of CustomEnrichRequest from a JSON string
custom_enrich_request_instance = CustomEnrichRequest.from_json(json)
# print the JSON string representation of the object
print(CustomEnrichRequest.to_json())

# convert the object into a dict
custom_enrich_request_dict = custom_enrich_request_instance.to_dict()
# create an instance of CustomEnrichRequest from a dict
custom_enrich_request_from_dict = CustomEnrichRequest.from_dict(custom_enrich_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


