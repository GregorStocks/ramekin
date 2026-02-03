# CreateShoppingListResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**ids** | **List[UUID]** |  | 

## Example

```python
from ramekin_client.models.create_shopping_list_response import CreateShoppingListResponse

# TODO update the JSON string below
json = "{}"
# create an instance of CreateShoppingListResponse from a JSON string
create_shopping_list_response_instance = CreateShoppingListResponse.from_json(json)
# print the JSON string representation of the object
print(CreateShoppingListResponse.to_json())

# convert the object into a dict
create_shopping_list_response_dict = create_shopping_list_response_instance.to_dict()
# create an instance of CreateShoppingListResponse from a dict
create_shopping_list_response_from_dict = CreateShoppingListResponse.from_dict(create_shopping_list_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


