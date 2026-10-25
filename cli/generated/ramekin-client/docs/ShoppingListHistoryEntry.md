# ShoppingListHistoryEntry

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**created_at** | **String** |  | 
**id** | [**uuid::Uuid**](uuid::Uuid.md) |  | 
**summary** | **String** | What the entry changed, e.g. \"Checked milk; deleted eggs and bread\". | 
**undoes_id** | Option<[**uuid::Uuid**](uuid::Uuid.md)> | The entry this one undid, when it's an undo. | [optional]
**undone_by_id** | Option<[**uuid::Uuid**](uuid::Uuid.md)> | The entry that undid this one. Only an entry without one can be undone. | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


