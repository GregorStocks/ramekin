# PrepareTextRecipeRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**text** | **str** | A whole recipe, typed or pasted as unstructured text. | 

## Example

```python
from ramekin_client.models.prepare_text_recipe_request import PrepareTextRecipeRequest

# TODO update the JSON string below
json = "{}"
# create an instance of PrepareTextRecipeRequest from a JSON string
prepare_text_recipe_request_instance = PrepareTextRecipeRequest.from_json(json)
# print the JSON string representation of the object
print(PrepareTextRecipeRequest.to_json())

# convert the object into a dict
prepare_text_recipe_request_dict = prepare_text_recipe_request_instance.to_dict()
# create an instance of PrepareTextRecipeRequest from a dict
prepare_text_recipe_request_from_dict = PrepareTextRecipeRequest.from_dict(prepare_text_recipe_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


