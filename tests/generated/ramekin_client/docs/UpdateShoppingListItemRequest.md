# UpdateShoppingListItemRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**amount** | **str** |  | [optional] 
**is_checked** | **bool** |  | [optional] 
**item** | **str** |  | [optional] 
**note** | **str** |  | [optional] 
**sort_order** | **int** |  | [optional] 

## Example

```python
from ramekin_client.models.update_shopping_list_item_request import UpdateShoppingListItemRequest

# TODO update the JSON string below
json = "{}"
# create an instance of UpdateShoppingListItemRequest from a JSON string
update_shopping_list_item_request_instance = UpdateShoppingListItemRequest.from_json(json)
# print the JSON string representation of the object
print(UpdateShoppingListItemRequest.to_json())

# convert the object into a dict
update_shopping_list_item_request_dict = update_shopping_list_item_request_instance.to_dict()
# create an instance of UpdateShoppingListItemRequest from a dict
update_shopping_list_item_request_from_dict = UpdateShoppingListItemRequest.from_dict(update_shopping_list_item_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


