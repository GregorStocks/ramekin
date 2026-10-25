# NormalizeProposalResponse


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**base** | [**NormalizedRecipe**](NormalizedRecipe.md) |  | 
**base_version_id** | **UUID** | The version the job started from. | 
**ops** | **List[str]** | The ops the job runs, in run order. | 
**proposed** | [**NormalizedRecipe**](NormalizedRecipe.md) | What accepting saves; absent until the job completes. | [optional] 
**status** | **str** | The job&#39;s status, as &#x60;GET /api/scrape/{id}&#x60; reports it. | 
**summary** | **str** | What an Edit with AI run says it changed, once it completes. | [optional] 

## Example

```python
from ramekin_client.models.normalize_proposal_response import NormalizeProposalResponse

# TODO update the JSON string below
json = "{}"
# create an instance of NormalizeProposalResponse from a JSON string
normalize_proposal_response_instance = NormalizeProposalResponse.from_json(json)
# print the JSON string representation of the object
print(NormalizeProposalResponse.to_json())

# convert the object into a dict
normalize_proposal_response_dict = normalize_proposal_response_instance.to_dict()
# create an instance of NormalizeProposalResponse from a dict
normalize_proposal_response_from_dict = NormalizeProposalResponse.from_dict(normalize_proposal_response_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


