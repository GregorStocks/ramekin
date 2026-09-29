# CalorieLine

One ingredient line's part of the estimate.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**calories** | [**CalorieRange**](CalorieRange.md) | Scaled calories when counted (zero when negligible). | [optional] 
**index** | **int** |  | 
**item** | **str** |  | 
**text** | **str** | Display text: \&quot;~120 kcal\&quot;, \&quot;Negligible\&quot;, \&quot;Not a food\&quot;, or why it couldn&#39;t be counted (\&quot;Not recognized\&quot;, \&quot;Amount unclear\&quot;). | 

## Example

```python
from ramekin_client.models.calorie_line import CalorieLine

# TODO update the JSON string below
json = "{}"
# create an instance of CalorieLine from a JSON string
calorie_line_instance = CalorieLine.from_json(json)
# print the JSON string representation of the object
print(CalorieLine.to_json())

# convert the object into a dict
calorie_line_dict = calorie_line_instance.to_dict()
# create an instance of CalorieLine from a dict
calorie_line_from_dict = CalorieLine.from_dict(calorie_line_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


