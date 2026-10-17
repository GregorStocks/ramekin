# ShoppingListFields

An item's writable fields. On `create`, `item` is required and the rest default (unchecked, sorted last). On `update`, only the fields present change. `amount`, `note` and `category_override` are cleared with \"\". The source recipe is set on create and never changes.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**amount** | **str** |  | [optional] 
**category_override** | **str** | User-selected aisle category; wins over the computed one. | [optional] 
**is_checked** | **bool** |  | [optional] 
**item** | **str** |  | [optional] 
**note** | **str** |  | [optional] 
**sort_order** | **int** |  | [optional] 
**source_recipe_id** | **UUID** |  | [optional] 
**source_recipe_title** | **str** |  | [optional] 

## Example

```python
from ramekin_client.models.shopping_list_fields import ShoppingListFields

# TODO update the JSON string below
json = "{}"
# create an instance of ShoppingListFields from a JSON string
shopping_list_fields_instance = ShoppingListFields.from_json(json)
# print the JSON string representation of the object
print(ShoppingListFields.to_json())

# convert the object into a dict
shopping_list_fields_dict = shopping_list_fields_instance.to_dict()
# create an instance of ShoppingListFields from a dict
shopping_list_fields_from_dict = ShoppingListFields.from_dict(shopping_list_fields_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


