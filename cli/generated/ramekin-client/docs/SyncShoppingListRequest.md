# SyncShoppingListRequest

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**cursor** | Option<**i64**> | `cursor` from the previous sync. Absent means a full sync. | [optional]
**ops** | Option<[**Vec<models::ShoppingListOp>**](ShoppingListOp.md)> | Writes to apply, in order, before reading changes. | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


