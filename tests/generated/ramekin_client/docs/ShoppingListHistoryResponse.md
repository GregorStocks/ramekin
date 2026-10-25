# ShoppingListHistoryResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**entries** | [**List[ShoppingListHistoryEntry]**](ShoppingListHistoryEntry.md) | Newest first. | 

## Example

```python
from ramekin_client.models.shopping_list_history_response import ShoppingListHistoryResponse

# TODO update the JSON string below
json = "{}"
# create an instance of ShoppingListHistoryResponse from a JSON string
shopping_list_history_response_instance = ShoppingListHistoryResponse.from_json(json)
# print the JSON string representation of the object
print(ShoppingListHistoryResponse.to_json())

# convert the object into a dict
shopping_list_history_response_dict = shopping_list_history_response_instance.to_dict()
# create an instance of ShoppingListHistoryResponse from a dict
shopping_list_history_response_from_dict = ShoppingListHistoryResponse.from_dict(shopping_list_history_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


