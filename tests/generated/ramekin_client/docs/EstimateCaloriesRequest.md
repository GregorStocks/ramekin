# EstimateCaloriesRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**ingredients** | [**List[Ingredient]**](Ingredient.md) |  | 
**scale** | **float** | Multiplier applied to the whole recipe, including its serving count. | 
**servings** | **str** | Original, unscaled servings text: a count or range, optionally with a \&quot;serves\&quot;, \&quot;servings\&quot;, \&quot;yield\&quot;, or \&quot;makes\&quot; prefix and a \&quot;servings\&quot;, \&quot;people\&quot;, \&quot;person(s)\&quot;, or \&quot;portion(s)\&quot; suffix (\&quot;Serves 4 to 6\&quot;, \&quot;Yield: 4\&quot;). A \&quot;makes\&quot; count needs a serving suffix; a yield of something else (\&quot;Makes 12 cookies\&quot;, \&quot;Makes 24\&quot;) gives no per-serving figure. | [optional] 

## Example

```python
from ramekin_client.models.estimate_calories_request import EstimateCaloriesRequest

# TODO update the JSON string below
json = "{}"
# create an instance of EstimateCaloriesRequest from a JSON string
estimate_calories_request_instance = EstimateCaloriesRequest.from_json(json)
# print the JSON string representation of the object
print(EstimateCaloriesRequest.to_json())

# convert the object into a dict
estimate_calories_request_dict = estimate_calories_request_instance.to_dict()
# create an instance of EstimateCaloriesRequest from a dict
estimate_calories_request_from_dict = EstimateCaloriesRequest.from_dict(estimate_calories_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


