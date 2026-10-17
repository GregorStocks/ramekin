# SyncShoppingListResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**category_order** | **Vec<String>** | Canonical category display order for grouping items; every item's `category` is guaranteed to appear in this list. | 
**changes** | [**Vec<models::SyncShoppingListItem>**](SyncShoppingListItem.md) | Live items changed at or after the request's `cursor`, this request's own writes included; every live item on a full resync. | 
**cursor** | **i64** | Pass as `cursor` on the next sync. | 
**deleted** | [**Vec<uuid::Uuid>**](uuid::Uuid.md) | Item IDs deleted at or after `cursor`. Empty on a full resync. | 
**full_resync** | **bool** | True when `changes` holds every live item: no cursor was sent, or it predates a restored backup, or came from another database. Drop cached items it omits. | 
**results** | [**Vec<models::OpResult>**](OpResult.md) | One per request op, in order. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


