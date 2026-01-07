# UpdateRecipeRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**cook_time** | **str** |  | [optional] 
**description** | **str** |  | [optional] 
**difficulty** | **str** |  | [optional] 
**ingredients** | [**List[Ingredient]**](Ingredient.md) |  | [optional] 
**instructions** | **str** |  | [optional] 
**notes** | **str** |  | [optional] 
**nutritional_info** | **str** |  | [optional] 
**photo_ids** | **List[UUID]** |  | [optional] 
**prep_time** | **str** |  | [optional] 
**rating** | **int** |  | [optional] 
**servings** | **str** |  | [optional] 
**source_name** | **str** |  | [optional] 
**source_url** | **str** |  | [optional] 
**tags** | **List[str]** |  | [optional] 
**title** | **str** |  | [optional] 
**total_time** | **str** |  | [optional] 

## Example

```python
from ramekin_client.models.update_recipe_request import UpdateRecipeRequest

# TODO update the JSON string below
json = "{}"
# create an instance of UpdateRecipeRequest from a JSON string
update_recipe_request_instance = UpdateRecipeRequest.from_json(json)
# print the JSON string representation of the object
print(UpdateRecipeRequest.to_json())

# convert the object into a dict
update_recipe_request_dict = update_recipe_request_instance.to_dict()
# create an instance of UpdateRecipeRequest from a dict
update_recipe_request_from_dict = UpdateRecipeRequest.from_dict(update_recipe_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


