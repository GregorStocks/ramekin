# ImportRecipeRequest

Request body for importing a recipe

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**extraction_method** | [**ImportExtractionMethod**](ImportExtractionMethod.md) | The extraction/import method used | 
**photo_ids** | **List[UUID]** | Photo IDs that have already been uploaded via POST /api/photos | 
**raw_recipe** | [**ImportRawRecipe**](ImportRawRecipe.md) | The raw recipe data (converted from import source by client) | 

## Example

```python
from ramekin_client.models.import_recipe_request import ImportRecipeRequest

# TODO update the JSON string below
json = "{}"
# create an instance of ImportRecipeRequest from a JSON string
import_recipe_request_instance = ImportRecipeRequest.from_json(json)
# print the JSON string representation of the object
print(ImportRecipeRequest.to_json())

# convert the object into a dict
import_recipe_request_dict = import_recipe_request_instance.to_dict()
# create an instance of ImportRecipeRequest from a dict
import_recipe_request_from_dict = ImportRecipeRequest.from_dict(import_recipe_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


