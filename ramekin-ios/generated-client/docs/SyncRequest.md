# SyncRequest

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**creates** | [SyncCreateItem] | Items created offline | [optional] 
**cursor** | **Int64** | &#x60;cursor&#x60; from the previous sync&#39;s response. Server returns changes at or after it, and takes precedence over &#x60;last_sync_at&#x60;. | [optional] 
**deletes** | **[UUID]** | IDs of items deleted offline | [optional] 
**lastSyncAt** | **Date** | Deprecated: use &#x60;cursor&#x60;. Server returns changes after this time when &#x60;cursor&#x60; is absent; with neither, it returns every item. | [optional] 
**updates** | [SyncUpdateItem] | Items updated offline | [optional] 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


