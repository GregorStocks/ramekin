# ListRecipesResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**pagination** | [**PaginationMetadata**](PaginationMetadata.md) |  | 
**recipes** | [**List[RecipeSummary]**](RecipeSummary.md) |  | 

## Example

```python
from ramekin_client.models.list_recipes_response import ListRecipesResponse

# TODO update the JSON string below
json = "{}"
# create an instance of ListRecipesResponse from a JSON string
list_recipes_response_instance = ListRecipesResponse.from_json(json)
# print the JSON string representation of the object
print(ListRecipesResponse.to_json())

# convert the object into a dict
list_recipes_response_dict = list_recipes_response_instance.to_dict()
# create an instance of ListRecipesResponse from a dict
list_recipes_response_from_dict = ListRecipesResponse.from_dict(list_recipes_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


