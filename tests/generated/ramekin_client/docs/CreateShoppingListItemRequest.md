# CreateShoppingListItemRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**amount** | **str** |  | [optional] 
**client_id** | **UUID** | Client-generated ID for offline sync | [optional] 
**item** | **str** |  | 
**note** | **str** |  | [optional] 
**source_recipe_id** | **UUID** |  | [optional] 
**source_recipe_title** | **str** |  | [optional] 

## Example

```python
from ramekin_client.models.create_shopping_list_item_request import CreateShoppingListItemRequest

# TODO update the JSON string below
json = "{}"
# create an instance of CreateShoppingListItemRequest from a JSON string
create_shopping_list_item_request_instance = CreateShoppingListItemRequest.from_json(json)
# print the JSON string representation of the object
print(CreateShoppingListItemRequest.to_json())

# convert the object into a dict
create_shopping_list_item_request_dict = create_shopping_list_item_request_instance.to_dict()
# create an instance of CreateShoppingListItemRequest from a dict
create_shopping_list_item_request_from_dict = CreateShoppingListItemRequest.from_dict(create_shopping_list_item_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


