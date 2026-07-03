# GetClientLogResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**app_version** | **str** |  | [optional] 
**content** | **str** |  | 
**created_at** | **datetime** |  | 
**id** | **UUID** |  | 
**os_info** | **str** |  | [optional] 
**platform** | **str** |  | 

## Example

```python
from ramekin_client.models.get_client_log_response import GetClientLogResponse

# TODO update the JSON string below
json = "{}"
# create an instance of GetClientLogResponse from a JSON string
get_client_log_response_instance = GetClientLogResponse.from_json(json)
# print the JSON string representation of the object
print(GetClientLogResponse.to_json())

# convert the object into a dict
get_client_log_response_dict = get_client_log_response_instance.to_dict()
# create an instance of GetClientLogResponse from a dict
get_client_log_response_from_dict = GetClientLogResponse.from_dict(get_client_log_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


