# SyncRecipe

Read-only recipe data needed to populate the iOS cache and mirror server search.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**calories_per_serving** | **str** | The estimate for display, as &#x60;RecipeSummary.calories_per_serving&#x60;. | [optional] 
**created_at** | **datetime** |  | 
**description** | **str** |  | [optional] 
**id** | **UUID** |  | 
**ingredient_match_text** | **str** | The stored ingredients JSON text, exactly as the database holds it — the exact haystack the server&#39;s bare-text search filter matches (JSON keys and syntax included). Local search must match against this string, not a re-encoding of &#x60;ingredients&#x60;, to reproduce server result membership. | 
**ingredients** | [**List[Ingredient]**](Ingredient.md) |  | 
**instructions** | **str** |  | 
**kcal_per_serving** | **float** | Estimated calories per serving: what &#x60;calories:&#x60; filters and the calorie sort compare. Null when there&#39;s no per-serving estimate. | [optional] 
**notes** | **str** |  | [optional] 
**photos** | [**List[SyncPhoto]**](SyncPhoto.md) | The recipe&#39;s photos, in order, leaving out deleted ones: what &#x60;photo_size:&#x60; and &#x60;photo_dim:&#x60; filters compare. | 
**rating** | **int** |  | [optional] 
**source_name** | **str** | What &#x60;source:&#x60; filters match. | [optional] 
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


