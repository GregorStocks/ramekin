# EnrichRequest

Request to enrich a recipe.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**enrichment_type** | **str** | The type of enrichment to apply (e.g., \&quot;normalize_ingredients\&quot;). | 
**recipe** | [**RecipeContent**](RecipeContent.md) | The recipe content to enrich. | 

## Example

```python
from ramekin_client.models.enrich_request import EnrichRequest

# TODO update the JSON string below
json = "{}"
# create an instance of EnrichRequest from a JSON string
enrich_request_instance = EnrichRequest.from_json(json)
# print the JSON string representation of the object
print(EnrichRequest.to_json())

# convert the object into a dict
enrich_request_dict = enrich_request_instance.to_dict()
# create an instance of EnrichRequest from a dict
enrich_request_from_dict = EnrichRequest.from_dict(enrich_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


