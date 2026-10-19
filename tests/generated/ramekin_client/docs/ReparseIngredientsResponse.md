# ReparseIngredientsResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**recipes_checked** | **int** | Live recipes looked at. | 
**recipes_updated** | **int** | Recipes that got a new \&quot;reparse\&quot; version because their ingredients changed. | 
**skipped_edited** | **int** | Recipes edited by hand since their last scrape, or changed while the re-parse ran. | 
**skipped_no_source** | **int** | Recipes with no saved scrape to re-read: typed in, or from text or a photo, or a scrape whose page or text can&#39;t be read. | 

## Example

```python
from ramekin_client.models.reparse_ingredients_response import ReparseIngredientsResponse

# TODO update the JSON string below
json = "{}"
# create an instance of ReparseIngredientsResponse from a JSON string
reparse_ingredients_response_instance = ReparseIngredientsResponse.from_json(json)
# print the JSON string representation of the object
print(ReparseIngredientsResponse.to_json())

# convert the object into a dict
reparse_ingredients_response_dict = reparse_ingredients_response_instance.to_dict()
# create an instance of ReparseIngredientsResponse from a dict
reparse_ingredients_response_from_dict = ReparseIngredientsResponse.from_dict(reparse_ingredients_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


