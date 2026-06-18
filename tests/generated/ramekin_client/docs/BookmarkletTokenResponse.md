# BookmarkletTokenResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**token** | **str** | A freshly minted, long-lived token scoped to the capture endpoints. Embed it in the bookmarklet; it does not expire and does not invalidate previously minted bookmarklet tokens. | 

## Example

```python
from ramekin_client.models.bookmarklet_token_response import BookmarkletTokenResponse

# TODO update the JSON string below
json = "{}"
# create an instance of BookmarkletTokenResponse from a JSON string
bookmarklet_token_response_instance = BookmarkletTokenResponse.from_json(json)
# print the JSON string representation of the object
print(BookmarkletTokenResponse.to_json())

# convert the object into a dict
bookmarklet_token_response_dict = bookmarklet_token_response_instance.to_dict()
# create an instance of BookmarkletTokenResponse from a dict
bookmarklet_token_response_from_dict = BookmarkletTokenResponse.from_dict(bookmarklet_token_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


