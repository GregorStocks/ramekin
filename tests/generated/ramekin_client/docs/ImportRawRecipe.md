# ImportRawRecipe

Raw recipe data for import (mirrors ramekin_core::RawRecipe)

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**categories** | **List[str]** |  | [optional] 
**cook_time** | **str** |  | [optional] 
**description** | **str** |  | [optional] 
**difficulty** | **str** |  | [optional] 
**image_urls** | **List[str]** | Image URLs found in the recipe (not used for imports with pre-uploaded photos) | [optional] 
**ingredients** | **str** | Ingredients as a newline-separated blob | 
**instructions** | **str** | Instructions as a blob (could be HTML or plain text) | 
**notes** | **str** |  | [optional] 
**nutritional_info** | **str** |  | [optional] 
**prep_time** | **str** |  | [optional] 
**rating** | **int** |  | [optional] 
**servings** | **str** |  | [optional] 
**source_name** | **str** |  | [optional] 
**source_url** | **str** | Source URL (optional for imports without a web source) | [optional] 
**title** | **str** |  | 
**total_time** | **str** |  | [optional] 

## Example

```python
from ramekin_client.models.import_raw_recipe import ImportRawRecipe

# TODO update the JSON string below
json = "{}"
# create an instance of ImportRawRecipe from a JSON string
import_raw_recipe_instance = ImportRawRecipe.from_json(json)
# print the JSON string representation of the object
print(ImportRawRecipe.to_json())

# convert the object into a dict
import_raw_recipe_dict = import_raw_recipe_instance.to_dict()
# create an instance of ImportRawRecipe from a dict
import_raw_recipe_from_dict = ImportRawRecipe.from_dict(import_raw_recipe_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


