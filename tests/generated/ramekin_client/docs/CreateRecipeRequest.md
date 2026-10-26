# CreateRecipeRequest


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
**photo_ids** | **List[UUID]** |  | [optional] 

## Example

```python
from ramekin_client.models.create_recipe_request import CreateRecipeRequest

# TODO update the JSON string below
json = "{}"
# create an instance of CreateRecipeRequest from a JSON string
create_recipe_request_instance = CreateRecipeRequest.from_json(json)
# print the JSON string representation of the object
print(CreateRecipeRequest.to_json())

# convert the object into a dict
create_recipe_request_dict = create_recipe_request_instance.to_dict()
# create an instance of CreateRecipeRequest from a dict
create_recipe_request_from_dict = CreateRecipeRequest.from_dict(create_recipe_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


