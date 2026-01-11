# EnrichResponse

Response from enrichment - the enhanced recipe object

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
**tags** | **List[str]** |  | 
**title** | **str** |  | 
**total_time** | **str** |  | [optional] 

## Example

```python
from ramekin_client.models.enrich_response import EnrichResponse

# TODO update the JSON string below
json = "{}"
# create an instance of EnrichResponse from a JSON string
enrich_response_instance = EnrichResponse.from_json(json)
# print the JSON string representation of the object
print(EnrichResponse.to_json())

# convert the object into a dict
enrich_response_dict = enrich_response_instance.to_dict()
# create an instance of EnrichResponse from a dict
enrich_response_from_dict = EnrichResponse.from_dict(enrich_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


