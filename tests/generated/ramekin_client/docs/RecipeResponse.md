# RecipeResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**created_at** | **datetime** |  | 
**description** | **str** |  | [optional] 
**id** | **UUID** |  | 
**ingredients** | [**List[Ingredient]**](Ingredient.md) |  | 
**instructions** | **str** |  | 
**photo_ids** | **List[UUID]** |  | 
**source_name** | **str** |  | [optional] 
**source_url** | **str** |  | [optional] 
**tags** | **List[str]** |  | 
**title** | **str** |  | 
**updated_at** | **datetime** |  | 

## Example

```python
from ramekin_client.models.recipe_response import RecipeResponse

# TODO update the JSON string below
json = "{}"
# create an instance of RecipeResponse from a JSON string
recipe_response_instance = RecipeResponse.from_json(json)
# print the JSON string representation of the object
print(RecipeResponse.to_json())

# convert the object into a dict
recipe_response_dict = recipe_response_instance.to_dict()
# create an instance of RecipeResponse from a dict
recipe_response_from_dict = RecipeResponse.from_dict(recipe_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


