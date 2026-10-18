# SyncShoppingListRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**cursor** | **int** | &#x60;cursor&#x60; from the previous sync. Absent means a full sync. | [optional] 
**ops** | [**List[ShoppingListOp]**](ShoppingListOp.md) | Writes to apply, in order, before reading changes. | [optional] 

## Example

```python
from ramekin_client.models.sync_shopping_list_request import SyncShoppingListRequest

# TODO update the JSON string below
json = "{}"
# create an instance of SyncShoppingListRequest from a JSON string
sync_shopping_list_request_instance = SyncShoppingListRequest.from_json(json)
# print the JSON string representation of the object
print(SyncShoppingListRequest.to_json())

# convert the object into a dict
sync_shopping_list_request_dict = sync_shopping_list_request_instance.to_dict()
# create an instance of SyncShoppingListRequest from a dict
sync_shopping_list_request_from_dict = SyncShoppingListRequest.from_dict(sync_shopping_list_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


