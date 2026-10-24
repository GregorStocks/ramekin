# ShoppingListHistoryEntry

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**createdAt** | **Date** |  | 
**id** | **UUID** |  | 
**summary** | **String** | What the entry changed, e.g. \&quot;Checked milk; deleted eggs and bread\&quot;. | 
**undoesId** | **UUID** | The entry this one undid, when it&#39;s an undo. | [optional] 
**undoneById** | **UUID** | The entry that undid this one. Only an entry without one can be undone. | [optional] 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


