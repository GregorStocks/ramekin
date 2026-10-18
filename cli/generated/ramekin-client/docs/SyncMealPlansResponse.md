# SyncMealPlansResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**changes** | [**Vec<models::SyncMealPlan>**](SyncMealPlan.md) | Live plans (of live recipes) changed at or after the request's `cursor`, this request's own writes and every refused op's live plan included; every live plan on a full resync. | 
**cursor** | **i64** | Pass as `cursor` on the next sync. | 
**deleted** | [**Vec<uuid::Uuid>**](uuid::Uuid.md) | Plan IDs deleted, or whose recipe was deleted, at or after `cursor`, and every refused op's plan that isn't live. Only the latter on a full resync. | 
**full_resync** | **bool** | True when `changes` holds every live plan: drop cached plans it omits. | 
**results** | [**Vec<models::OpResult>**](OpResult.md) | One per request op, in order. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


