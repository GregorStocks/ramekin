# ShoppingListHistoryEntry


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**created_at** | **datetime** |  | 
**id** | **UUID** |  | 
**summary** | **str** | What the entry changed, e.g. \&quot;Checked milk; deleted eggs and bread\&quot;. | 
**undoes_id** | **UUID** | The entry this one undid, when it&#39;s an undo. | [optional] 
**undone_by_id** | **UUID** | The entry that undid this one. Only an entry without one can be undone. | [optional] 

## Example

```python
from ramekin_client.models.shopping_list_history_entry import ShoppingListHistoryEntry

# TODO update the JSON string below
json = "{}"
# create an instance of ShoppingListHistoryEntry from a JSON string
shopping_list_history_entry_instance = ShoppingListHistoryEntry.from_json(json)
# print the JSON string representation of the object
print(ShoppingListHistoryEntry.to_json())

# convert the object into a dict
shopping_list_history_entry_dict = shopping_list_history_entry_instance.to_dict()
# create an instance of ShoppingListHistoryEntry from a dict
shopping_list_history_entry_from_dict = ShoppingListHistoryEntry.from_dict(shopping_list_history_entry_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


