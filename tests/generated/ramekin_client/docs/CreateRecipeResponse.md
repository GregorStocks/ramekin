# CreateRecipeResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**id** | **UUID** |  | 

## Example

```python
from ramekin_client.models.create_recipe_response import CreateRecipeResponse

# TODO update the JSON string below
json = "{}"
# create an instance of CreateRecipeResponse from a JSON string
create_recipe_response_instance = CreateRecipeResponse.from_json(json)
# print the JSON string representation of the object
print(CreateRecipeResponse.to_json())

# convert the object into a dict
create_recipe_response_dict = create_recipe_response_instance.to_dict()
# create an instance of CreateRecipeResponse from a dict
create_recipe_response_from_dict = CreateRecipeResponse.from_dict(create_recipe_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


