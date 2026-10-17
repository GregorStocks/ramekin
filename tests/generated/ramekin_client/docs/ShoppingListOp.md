# ShoppingListOp


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**fields** | [**ShoppingListFields**](ShoppingListFields.md) |  | [optional] 
**id** | **UUID** | The item&#39;s id. A create chooses it. | 
**op** | [**OpKind**](OpKind.md) |  | 

## Example

```python
from ramekin_client.models.shopping_list_op import ShoppingListOp

# TODO update the JSON string below
json = "{}"
# create an instance of ShoppingListOp from a JSON string
shopping_list_op_instance = ShoppingListOp.from_json(json)
# print the JSON string representation of the object
print(ShoppingListOp.to_json())

# convert the object into a dict
shopping_list_op_dict = shopping_list_op_instance.to_dict()
# create an instance of ShoppingListOp from a dict
shopping_list_op_from_dict = ShoppingListOp.from_dict(shopping_list_op_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


