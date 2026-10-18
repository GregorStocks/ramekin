# SyncTagsResponse

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**changes** | [SyncTag] | Live tags created, renamed or revived at or after &#x60;cursor&#x60;; every live tag on a full resync. | 
**cursor** | **Int64** | Pass as &#x60;cursor&#x60; on the next sync. | 
**deleted** | **[UUID]** | Tag IDs deleted at or after &#x60;cursor&#x60;. Empty on a full resync. | 
**fullResync** | **Bool** | True when &#x60;changes&#x60; holds every live tag: drop cached tags it omits. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


