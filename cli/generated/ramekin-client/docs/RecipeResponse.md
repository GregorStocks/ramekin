# RecipeResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**cook_time** | Option<**String**> |  | [optional]
**created_at** | **String** |  | 
**description** | Option<**String**> |  | [optional]
**difficulty** | Option<**String**> |  | [optional]
**id** | [**uuid::Uuid**](uuid::Uuid.md) |  | 
**ingredients** | [**Vec<models::Ingredient>**](Ingredient.md) |  | 
**instructions** | **String** |  | 
**notes** | Option<**String**> |  | [optional]
**nutritional_info** | Option<**String**> |  | [optional]
**photo_ids** | [**Vec<uuid::Uuid>**](uuid::Uuid.md) |  | 
**prep_time** | Option<**String**> |  | [optional]
**rating** | Option<**i32**> |  | [optional]
**servings** | Option<**String**> |  | [optional]
**source_name** | Option<**String**> |  | [optional]
**source_url** | Option<**String**> |  | [optional]
**tags** | **Vec<String>** |  | 
**title** | **String** |  | 
**total_time** | Option<**String**> |  | [optional]
**updated_at** | **String** | When viewing a specific version, this is the version's created_at | 
**version_id** | [**uuid::Uuid**](uuid::Uuid.md) | Version metadata | 
**version_source** | **String** |  | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


