# ListClientLogsResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**uploads** | [**List[ClientLogSummary]**](ClientLogSummary.md) |  | 

## Example

```python
from ramekin_client.models.list_client_logs_response import ListClientLogsResponse

# TODO update the JSON string below
json = "{}"
# create an instance of ListClientLogsResponse from a JSON string
list_client_logs_response_instance = ListClientLogsResponse.from_json(json)
# print the JSON string representation of the object
print(ListClientLogsResponse.to_json())

# convert the object into a dict
list_client_logs_response_dict = list_client_logs_response_instance.to_dict()
# create an instance of ListClientLogsResponse from a dict
list_client_logs_response_from_dict = ListClientLogsResponse.from_dict(list_client_logs_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


