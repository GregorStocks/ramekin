# NormalizeProposalResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**base** | [**models::NormalizedRecipe**](NormalizedRecipe.md) |  | 
**base_version_id** | [**uuid::Uuid**](uuid::Uuid.md) | The version the job started from. | 
**ops** | **Vec<String>** | The ops the job runs, in run order. | 
**proposed** | Option<[**models::NormalizedRecipe**](NormalizedRecipe.md)> | What accepting saves; absent until the job completes. | [optional]
**status** | **String** | The job's status, as `GET /api/scrape/{id}` reports it. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


