# UnauthedPingResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**message** | **str** |  | 

## Example

```python
from ramekin_client.models.unauthed_ping_response import UnauthedPingResponse

# TODO update the JSON string below
json = "{}"
# create an instance of UnauthedPingResponse from a JSON string
unauthed_ping_response_instance = UnauthedPingResponse.from_json(json)
# print the JSON string representation of the object
print(UnauthedPingResponse.to_json())

# convert the object into a dict
unauthed_ping_response_dict = unauthed_ping_response_instance.to_dict()
# create an instance of UnauthedPingResponse from a dict
unauthed_ping_response_from_dict = UnauthedPingResponse.from_dict(unauthed_ping_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


