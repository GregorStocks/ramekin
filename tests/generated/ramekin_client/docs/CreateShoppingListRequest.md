# CreateShoppingListRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**items** | [**List[CreateShoppingListItemRequest]**](CreateShoppingListItemRequest.md) |  | 

## Example

```python
from ramekin_client.models.create_shopping_list_request import CreateShoppingListRequest

# TODO update the JSON string below
json = "{}"
# create an instance of CreateShoppingListRequest from a JSON string
create_shopping_list_request_instance = CreateShoppingListRequest.from_json(json)
# print the JSON string representation of the object
print(CreateShoppingListRequest.to_json())

# convert the object into a dict
create_shopping_list_request_dict = create_shopping_list_request_instance.to_dict()
# create an instance of CreateShoppingListRequest from a dict
create_shopping_list_request_from_dict = CreateShoppingListRequest.from_dict(create_shopping_list_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


