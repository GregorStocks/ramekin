# ShoppingGroup

A recipe's ingredients that share a shopping-list category.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**category** | **str** |  | 
**ingredient_indices** | **List[int]** | Indices into the recipe&#39;s ingredients, in ingredient order. | 

## Example

```python
from ramekin_client.models.shopping_group import ShoppingGroup

# TODO update the JSON string below
json = "{}"
# create an instance of ShoppingGroup from a JSON string
shopping_group_instance = ShoppingGroup.from_json(json)
# print the JSON string representation of the object
print(ShoppingGroup.to_json())

# convert the object into a dict
shopping_group_dict = shopping_group_instance.to_dict()
# create an instance of ShoppingGroup from a dict
shopping_group_from_dict = ShoppingGroup.from_dict(shopping_group_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


