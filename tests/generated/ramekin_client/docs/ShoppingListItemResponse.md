# ShoppingListItemResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**amount** | **str** |  | [optional] 
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
from ramekin_client.models.shopping_list_item_response import ShoppingListItemResponse

# TODO update the JSON string below
json = "{}"
# create an instance of ShoppingListItemResponse from a JSON string
shopping_list_item_response_instance = ShoppingListItemResponse.from_json(json)
# print the JSON string representation of the object
print(ShoppingListItemResponse.to_json())

# convert the object into a dict
shopping_list_item_response_dict = shopping_list_item_response_instance.to_dict()
# create an instance of ShoppingListItemResponse from a dict
shopping_list_item_response_from_dict = ShoppingListItemResponse.from_dict(shopping_list_item_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


