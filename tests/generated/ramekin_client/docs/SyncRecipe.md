# SyncRecipe

Read-only recipe data needed to populate the iOS cache and mirror server search.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**created_at** | **datetime** |  | 
**description** | **str** |  | [optional] 
**id** | **UUID** |  | 
**ingredients** | [**List[Ingredient]**](Ingredient.md) |  | 
**instructions** | **str** |  | 
**notes** | **str** |  | [optional] 
**rating** | **int** |  | [optional] 
**tags** | **List[str]** |  | 
**thumbnail_photo_id** | **UUID** |  | [optional] 
**title** | **str** |  | 
**updated_at** | **datetime** |  | 

## Example

```python
from ramekin_client.models.sync_recipe import SyncRecipe

# TODO update the JSON string below
json = "{}"
# create an instance of SyncRecipe from a JSON string
sync_recipe_instance = SyncRecipe.from_json(json)
# print the JSON string representation of the object
print(SyncRecipe.to_json())

# convert the object into a dict
sync_recipe_dict = sync_recipe_instance.to_dict()
# create an instance of SyncRecipe from a dict
sync_recipe_from_dict = SyncRecipe.from_dict(sync_recipe_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


