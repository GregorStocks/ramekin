# SyncShoppingListItem

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**amount** | **String** |  | [optional] 
**category** | **String** | Aisle category for grouping (override when set, otherwise computed). | 
**categoryOverride** | **String** | User-selected category override; when set, it wins over computed category. | [optional] 
**changeSeq** | **Int64** | Sync stamp of the row&#39;s last change. A cached copy with a higher one is newer than this row. | 
**computedCategory** | **String** | Category computed from the item name before applying any override. | 
**id** | **UUID** |  | 
**isChecked** | **Bool** |  | 
**item** | **String** |  | 
**note** | **String** |  | [optional] 
**sortOrder** | **Int** |  | 
**sourceRecipeId** | **UUID** |  | [optional] 
**sourceRecipeTitle** | **String** |  | [optional] 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


