# PrepareTextRecipeResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**content** | [**RecipeContent**](RecipeContent.md) |  | 
**raw_ingredients** | **str** | Editable ingredient lines. Send these as raw_ingredients when saving. | 
**warnings** | **List[str]** |  | 

## Example

```python
from ramekin_client.models.prepare_text_recipe_response import PrepareTextRecipeResponse

# TODO update the JSON string below
json = "{}"
# create an instance of PrepareTextRecipeResponse from a JSON string
prepare_text_recipe_response_instance = PrepareTextRecipeResponse.from_json(json)
# print the JSON string representation of the object
print(PrepareTextRecipeResponse.to_json())

# convert the object into a dict
prepare_text_recipe_response_dict = prepare_text_recipe_response_instance.to_dict()
# create an instance of PrepareTextRecipeResponse from a dict
prepare_text_recipe_response_from_dict = PrepareTextRecipeResponse.from_dict(prepare_text_recipe_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


