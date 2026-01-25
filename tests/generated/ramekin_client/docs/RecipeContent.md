# RecipeContent

Core recipe content - all fields that can be enriched by AI. Used for enrichment APIs and recipe import.

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
from ramekin_client.models.recipe_content import RecipeContent

# TODO update the JSON string below
json = "{}"
# create an instance of RecipeContent from a JSON string
recipe_content_instance = RecipeContent.from_json(json)
# print the JSON string representation of the object
print(RecipeContent.to_json())

# convert the object into a dict
recipe_content_dict = recipe_content_instance.to_dict()
# create an instance of RecipeContent from a dict
recipe_content_from_dict = RecipeContent.from_dict(recipe_content_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


