# ShoppingListItemResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**amount** | Option<**String**> |  | [optional]
**category** | **String** | Aisle category for grouping (override when set, otherwise computed). | 
**category_override** | Option<**String**> | User-selected category override; when set, it wins over computed category. | [optional]
**computed_category** | **String** | Category computed from the item name before applying any override. | 
**id** | [**uuid::Uuid**](uuid::Uuid.md) |  | 
**is_checked** | **bool** |  | 
**item** | **String** |  | 
**note** | Option<**String**> |  | [optional]
**sort_order** | **i32** |  | 
**source_recipe_id** | Option<[**uuid::Uuid**](uuid::Uuid.md)> |  | [optional]
**source_recipe_title** | Option<**String**> |  | [optional]
**updated_at** | **String** |  | 
**version** | **i32** |  | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


