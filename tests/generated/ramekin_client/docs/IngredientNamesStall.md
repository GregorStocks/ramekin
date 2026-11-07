# IngredientNamesStall

The background resolver's last pass failed for a reason that isn't any one name's (the provider is down, rate-limited, or misconfigured), so pending names wait for it to try again.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**error** | **str** |  | 
**next_attempt_at** | **datetime** | When the resolver tries again at the latest. A newly queued name or a Retry can make it try sooner. | 

## Example

```python
from ramekin_client.models.ingredient_names_stall import IngredientNamesStall

# TODO update the JSON string below
json = "{}"
# create an instance of IngredientNamesStall from a JSON string
ingredient_names_stall_instance = IngredientNamesStall.from_json(json)
# print the JSON string representation of the object
print(IngredientNamesStall.to_json())

# convert the object into a dict
ingredient_names_stall_dict = ingredient_names_stall_instance.to_dict()
# create an instance of IngredientNamesStall from a dict
ingredient_names_stall_from_dict = IngredientNamesStall.from_dict(ingredient_names_stall_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


