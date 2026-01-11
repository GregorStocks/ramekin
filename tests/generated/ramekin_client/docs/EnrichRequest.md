# EnrichRequest

Request body for enrichment - a recipe object to enhance

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**cook_time** | **str** |  | [optional] 
**description** | **str** |  | [optional] 
**difficulty** | **str** |  | [optional] 
**ingredients** | [**List[Ingredient]**](Ingredient.md) |  | 
**instructions** | **str** |  | 
**notes** | **str** |  | [optional] 
**nutritional_info** | **str** |  | [optional] 
**prep_time** | **str** |  | [optional] 
**rating** | **int** |  | [optional] 
**servings** | **str** |  | [optional] 
**source_name** | **str** |  | [optional] 
**source_url** | **str** |  | [optional] 
**tags** | **List[str]** |  | [optional] 
**title** | **str** |  | 
**total_time** | **str** |  | [optional] 

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


