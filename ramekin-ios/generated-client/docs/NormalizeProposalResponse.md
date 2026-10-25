# NormalizeProposalResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**base** | [**NormalizedRecipe**](NormalizedRecipe.md) |  | 
**baseVersionId** | **UUID** | The version the job started from. | 
**ops** | **[String]** | The ops the job runs, in run order. | 
**proposed** | [**NormalizedRecipe**](NormalizedRecipe.md) | What accepting saves; absent until the job completes. | [optional] 
**status** | **String** | The job&#39;s status, as &#x60;GET /api/scrape/{id}&#x60; reports it. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


