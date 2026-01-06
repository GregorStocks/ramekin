# RecipeSummary


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**created_at** | **datetime** |  | 
**description** | **str** |  | [optional] 
**id** | **UUID** |  | 
**tags** | **List[str]** |  | 
**thumbnail_photo_id** | **UUID** | Photo ID of the first photo (thumbnail), if any | [optional] 
**title** | **str** |  | 
**updated_at** | **datetime** |  | 

## Example

```python
from ramekin_client.models.recipe_summary import RecipeSummary

# TODO update the JSON string below
json = "{}"
# create an instance of RecipeSummary from a JSON string
recipe_summary_instance = RecipeSummary.from_json(json)
# print the JSON string representation of the object
print(RecipeSummary.to_json())

# convert the object into a dict
recipe_summary_dict = recipe_summary_instance.to_dict()
# create an instance of RecipeSummary from a dict
recipe_summary_from_dict = RecipeSummary.from_dict(recipe_summary_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


