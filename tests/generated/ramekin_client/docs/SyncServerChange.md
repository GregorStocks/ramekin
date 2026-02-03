# SyncServerChange


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**amount** | **str** |  | [optional] 
**category** | **str** | Computed aisle category for grouping (e.g., \&quot;Produce\&quot;, \&quot;Dairy &amp; Eggs\&quot;) | 
**id** | **UUID** |  | 
**is_checked** | **bool** |  | 
**item** | **str** |  | 
**note** | **str** |  | [optional] 
**sort_order** | **int** |  | 
**source_recipe_id** | **UUID** |  | [optional] 
**source_recipe_title** | **str** |  | [optional] 
**updated_at** | **datetime** |  | 
**version** | **int** |  | 

## Example

```python
from ramekin_client.models.sync_server_change import SyncServerChange

# TODO update the JSON string below
json = "{}"
# create an instance of SyncServerChange from a JSON string
sync_server_change_instance = SyncServerChange.from_json(json)
# print the JSON string representation of the object
print(SyncServerChange.to_json())

# convert the object into a dict
sync_server_change_dict = sync_server_change_instance.to_dict()
# create an instance of SyncServerChange from a dict
sync_server_change_from_dict = SyncServerChange.from_dict(sync_server_change_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


