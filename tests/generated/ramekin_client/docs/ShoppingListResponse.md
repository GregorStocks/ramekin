# ShoppingListResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**items** | [**List[ShoppingListItemResponse]**](ShoppingListItemResponse.md) |  | 

## Example

```python
from ramekin_client.models.shopping_list_response import ShoppingListResponse

# TODO update the JSON string below
json = "{}"
# create an instance of ShoppingListResponse from a JSON string
shopping_list_response_instance = ShoppingListResponse.from_json(json)
# print the JSON string representation of the object
print(ShoppingListResponse.to_json())

# convert the object into a dict
shopping_list_response_dict = shopping_list_response_instance.to_dict()
# create an instance of ShoppingListResponse from a dict
shopping_list_response_from_dict = ShoppingListResponse.from_dict(shopping_list_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


