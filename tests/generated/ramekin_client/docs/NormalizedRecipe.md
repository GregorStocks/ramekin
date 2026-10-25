# NormalizedRecipe

A recipe's content and photos: what a normalize run starts from, and what it proposes.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**content** | [**RecipeContent**](RecipeContent.md) |  | 
**photo_ids** | **List[UUID]** |  | 

## Example

```python
from ramekin_client.models.normalized_recipe import NormalizedRecipe

# TODO update the JSON string below
json = "{}"
# create an instance of NormalizedRecipe from a JSON string
normalized_recipe_instance = NormalizedRecipe.from_json(json)
# print the JSON string representation of the object
print(NormalizedRecipe.to_json())

# convert the object into a dict
normalized_recipe_dict = normalized_recipe_instance.to_dict()
# create an instance of NormalizedRecipe from a dict
normalized_recipe_from_dict = NormalizedRecipe.from_dict(normalized_recipe_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


