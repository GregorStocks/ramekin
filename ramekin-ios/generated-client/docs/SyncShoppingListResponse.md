# SyncShoppingListResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**categoryOrder** | **[String]** | Canonical category display order for grouping items; every item&#39;s &#x60;category&#x60; is guaranteed to appear in this list. | 
**changes** | [SyncShoppingListItem] | Live items changed at or after the request&#39;s &#x60;cursor&#x60;, this request&#39;s own writes included; every live item on a full resync. | 
**cursor** | **Int64** | Pass as &#x60;cursor&#x60; on the next sync. | 
**deleted** | **[UUID]** | Item IDs deleted at or after &#x60;cursor&#x60;, and every refused op&#39;s item that isn&#39;t live. Only the latter on a full resync. | 
**fullResync** | **Bool** | True when &#x60;changes&#x60; holds every live item: no cursor was sent, or it predates a restored backup, or came from another database. Drop cached items it omits. | 
**results** | [OpResult] | One per request op, in order. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


