# EditRecipeRequest

Request body for Edit with AI.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**instruction** | **str** |  | 
**photo_ids** | **List[UUID]** |  | [optional] 
**recipe** | [**RecipeContent**](RecipeContent.md) |  | 

## Example

```python
from ramekin_client.models.edit_recipe_request import EditRecipeRequest

# TODO update the JSON string below
json = "{}"
# create an instance of EditRecipeRequest from a JSON string
edit_recipe_request_instance = EditRecipeRequest.from_json(json)
# print the JSON string representation of the object
print(EditRecipeRequest.to_json())

# convert the object into a dict
edit_recipe_request_dict = edit_recipe_request_instance.to_dict()
# create an instance of EditRecipeRequest from a dict
edit_recipe_request_from_dict = EditRecipeRequest.from_dict(edit_recipe_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


