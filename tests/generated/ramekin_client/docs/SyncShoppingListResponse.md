# SyncShoppingListResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**category_order** | **List[str]** | Canonical category display order for grouping items; every item&#39;s &#x60;category&#x60; is guaranteed to appear in this list. | 
**changes** | [**List[SyncShoppingListItem]**](SyncShoppingListItem.md) | Live items changed at or after the request&#39;s &#x60;cursor&#x60;, this request&#39;s own writes included; every live item on a full resync. | 
**cursor** | **int** | Pass as &#x60;cursor&#x60; on the next sync. | 
**deleted** | **List[UUID]** | Item IDs deleted at or after &#x60;cursor&#x60;. Empty on a full resync. | 
**full_resync** | **bool** | True when &#x60;changes&#x60; holds every live item: no cursor was sent, or it predates a restored backup, or came from another database. Drop cached items it omits. | 
**results** | [**List[OpResult]**](OpResult.md) | One per request op, in order. | 

## Example

```python
from ramekin_client.models.sync_shopping_list_response import SyncShoppingListResponse

# TODO update the JSON string below
json = "{}"
# create an instance of SyncShoppingListResponse from a JSON string
sync_shopping_list_response_instance = SyncShoppingListResponse.from_json(json)
# print the JSON string representation of the object
print(SyncShoppingListResponse.to_json())

# convert the object into a dict
sync_shopping_list_response_dict = sync_shopping_list_response_instance.to_dict()
# create an instance of SyncShoppingListResponse from a dict
sync_shopping_list_response_from_dict = SyncShoppingListResponse.from_dict(sync_shopping_list_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


