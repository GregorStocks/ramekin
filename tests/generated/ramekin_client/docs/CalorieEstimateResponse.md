# CalorieEstimateResponse

Every display string is final; clients render them as-is.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**database_version** | **str** | Identifies the pinned source data, aliases, and calculation rules. | 
**headline** | **str** | The main line: \&quot;~520 kcal per serving\&quot;, \&quot;At least ~3,100 kcal for the whole recipe\&quot;, or \&quot;Not enough ingredient data to estimate calories\&quot;. | 
**known_calories** | [**CalorieRange**](CalorieRange.md) | Null when nothing was counted. A lower bound when status is partial. | [optional] 
**lines** | [**List[CalorieLine]**](CalorieLine.md) | The breakdown, one entry per ingredient in order. | 
**not_counted** | **List[str]** | Ingredients the figures leave out, in recipe order: those given no amount (even in a complete estimate), and for a partial estimate the ones that couldn&#39;t be counted. | 
**per_serving_calories** | [**CalorieRange**](CalorieRange.md) |  | [optional] 
**resolving** | **bool** | Some ingredient names are still being recognized, or weights estimated, in the background; ask again shortly for an estimate that includes them. | 
**secondary** | **str** | Shown under the headline when present. | [optional] 
**status** | [**CalorieStatus**](CalorieStatus.md) |  | 

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


