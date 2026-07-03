# ClientLogSummary


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**app_version** | **str** |  | [optional] 
**content_length** | **int** |  | 
**created_at** | **datetime** |  | 
**id** | **UUID** |  | 
**os_info** | **str** |  | [optional] 
**platform** | **str** |  | 

## Example

```python
from ramekin_client.models.client_log_summary import ClientLogSummary

# TODO update the JSON string below
json = "{}"
# create an instance of ClientLogSummary from a JSON string
client_log_summary_instance = ClientLogSummary.from_json(json)
# print the JSON string representation of the object
print(ClientLogSummary.to_json())

# convert the object into a dict
client_log_summary_dict = client_log_summary_instance.to_dict()
# create an instance of ClientLogSummary from a dict
client_log_summary_from_dict = ClientLogSummary.from_dict(client_log_summary_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


