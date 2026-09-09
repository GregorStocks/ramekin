# CalorieEstimateResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**database_version** | **str** | Identifies the pinned source data, aliases, and calculation rules. | 
**known_calories** | [**CalorieRange**](CalorieRange.md) | Null when no ingredient could be estimated. Otherwise a subtotal that may be partial. | [optional] 
**per_serving_calories** | [**CalorieRange**](CalorieRange.md) |  | [optional] 
**per_serving_summary** | **str** |  | [optional] 
**summary** | **str** |  | 
**unknown_ingredients** | [**List[UnknownCalorieIngredient]**](UnknownCalorieIngredient.md) |  | 

## Example

```python
from ramekin_client.models.calorie_estimate_response import CalorieEstimateResponse

# TODO update the JSON string below
json = "{}"
# create an instance of CalorieEstimateResponse from a JSON string
calorie_estimate_response_instance = CalorieEstimateResponse.from_json(json)
# print the JSON string representation of the object
print(CalorieEstimateResponse.to_json())

# convert the object into a dict
calorie_estimate_response_dict = calorie_estimate_response_instance.to_dict()
# create an instance of CalorieEstimateResponse from a dict
calorie_estimate_response_from_dict = CalorieEstimateResponse.from_dict(calorie_estimate_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


