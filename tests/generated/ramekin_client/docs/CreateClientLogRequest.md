# CreateClientLogRequest


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**app_version** | **str** |  | [optional] 
**content** | **str** |  | 
**os_info** | **str** |  | [optional] 
**platform** | **str** |  | 

## Example

```python
from ramekin_client.models.create_client_log_request import CreateClientLogRequest

# TODO update the JSON string below
json = "{}"
# create an instance of CreateClientLogRequest from a JSON string
create_client_log_request_instance = CreateClientLogRequest.from_json(json)
# print the JSON string representation of the object
print(CreateClientLogRequest.to_json())

# convert the object into a dict
create_client_log_request_dict = create_client_log_request_instance.to_dict()
# create an instance of CreateClientLogRequest from a dict
create_client_log_request_from_dict = CreateClientLogRequest.from_dict(create_client_log_request_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


