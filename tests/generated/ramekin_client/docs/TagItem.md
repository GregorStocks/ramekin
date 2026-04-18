# TagItem


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**created_at** | **datetime** |  | 
**id** | **UUID** |  | 
**name** | **str** |  | 
**namespace** | **str** | Namespace portion for &#x60;namespace:value&#x60;-shaped names, else null. | [optional] 
**recipe_count** | **int** | Number of recipes using this tag | 
**value** | **str** | Value portion. Equals &#x60;name&#x60; for flat tags. | 

## Example

```python
from ramekin_client.models.tag_item import TagItem

# TODO update the JSON string below
json = "{}"
# create an instance of TagItem from a JSON string
tag_item_instance = TagItem.from_json(json)
# print the JSON string representation of the object
print(TagItem.to_json())

# convert the object into a dict
tag_item_dict = tag_item_instance.to_dict()
# create an instance of TagItem from a dict
tag_item_from_dict = TagItem.from_dict(tag_item_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


