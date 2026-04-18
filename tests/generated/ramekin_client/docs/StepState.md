# StepState

A single pipeline step's state for the status API response.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**duration_ms** | **int** |  | [optional] 
**error** | **str** |  | [optional] 
**finished_at** | **datetime** |  | [optional] 
**has_output** | **bool** |  | 
**name** | **str** |  | 
**started_at** | **datetime** |  | [optional] 
**status** | **str** | One of \&quot;pending\&quot;, \&quot;running\&quot;, \&quot;completed\&quot;, \&quot;failed\&quot;, \&quot;skipped\&quot;. | 
**summary** | **str** |  | [optional] 

## Example

```python
from ramekin_client.models.step_state import StepState

# TODO update the JSON string below
json = "{}"
# create an instance of StepState from a JSON string
step_state_instance = StepState.from_json(json)
# print the JSON string representation of the object
print(StepState.to_json())

# convert the object into a dict
step_state_dict = step_state_instance.to_dict()
# create an instance of StepState from a dict
step_state_from_dict = StepState.from_dict(step_state_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


