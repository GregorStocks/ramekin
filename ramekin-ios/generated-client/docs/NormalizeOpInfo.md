# NormalizeOpInfo

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**available** | **Bool** | Whether the op can run on the recipe asked about; always true without a recipe. | 
**defaultOnImport** | **Bool** | Whether imports and scrapes run the op automatically. | 
**group** | [**NormalizeOpGroupResponse**](NormalizeOpGroupResponse.md) |  | 
**id** | **String** |  | 
**implies** | **[String]** | Ops that always run with this one. | 
**label** | **String** |  | 
**runsAfter** | **[String]** | Ops this one runs after when both run. | 
**unavailableReason** | **String** |  | [optional] 
**usesAi** | **Bool** |  | 
**writes** | **[String]** | Recipe fields the op can change. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


