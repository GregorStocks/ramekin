# NormalizeOpInfo

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**available** | **bool** | Whether the op can run on the recipe asked about; always true without a recipe. | 
**default_on_import** | **bool** | Whether imports and scrapes run the op automatically. | 
**group** | [**models::NormalizeOpGroupResponse**](NormalizeOpGroupResponse.md) |  | 
**id** | **String** |  | 
**implies** | **Vec<String>** | Ops that always run with this one. | 
**label** | **String** |  | 
**runs_after** | **Vec<String>** | Ops this one runs after when both run. | 
**unavailable_reason** | Option<**String**> |  | [optional]
**uses_ai** | **bool** |  | 
**writes** | **Vec<String>** | Recipe fields the op can change. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


