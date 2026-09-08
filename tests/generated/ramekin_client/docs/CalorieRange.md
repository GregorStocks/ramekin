# CalorieRange


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**max** | **float** |  | 
**min** | **float** |  | 

## Example

```python
from ramekin_client.models.calorie_range import CalorieRange

# TODO update the JSON string below
json = "{}"
# create an instance of CalorieRange from a JSON string
calorie_range_instance = CalorieRange.from_json(json)
# print the JSON string representation of the object
print(CalorieRange.to_json())

# convert the object into a dict
calorie_range_dict = calorie_range_instance.to_dict()
# create an instance of CalorieRange from a dict
calorie_range_from_dict = CalorieRange.from_dict(calorie_range_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


