# SyncMealPlansResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**changes** | [SyncMealPlan] | Live plans (of live recipes) changed at or after the request&#39;s &#x60;cursor&#x60;, this request&#39;s own writes and any plan an op couldn&#39;t update included; every live plan on a full resync. | 
**cursor** | **Int64** | Pass as &#x60;cursor&#x60; on the next sync. | 
**deleted** | **[UUID]** | Plan IDs deleted, or whose recipe was deleted, at or after &#x60;cursor&#x60;. Empty on a full resync. | 
**fullResync** | **Bool** | True when &#x60;changes&#x60; holds every live plan: drop cached plans it omits. | 
**results** | [OpResult] | One per request op, in order. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


