# ImportRecipeResponse

Response from recipe import

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**job_id** | **UUID** | The created job ID | 
**status** | **str** | Current job status | 

## Example

```python
from ramekin_client.models.import_recipe_response import ImportRecipeResponse

# TODO update the JSON string below
json = "{}"
# create an instance of ImportRecipeResponse from a JSON string
import_recipe_response_instance = ImportRecipeResponse.from_json(json)
# print the JSON string representation of the object
print(ImportRecipeResponse.to_json())

# convert the object into a dict
import_recipe_response_dict = import_recipe_response_instance.to_dict()
# create an instance of ImportRecipeResponse from a dict
import_recipe_response_from_dict = ImportRecipeResponse.from_dict(import_recipe_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


