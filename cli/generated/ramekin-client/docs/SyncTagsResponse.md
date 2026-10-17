# SyncTagsResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**changes** | [**Vec<models::SyncTag>**](SyncTag.md) | Live tags created, renamed or revived at or after `cursor`; every live tag on a full resync. | 
**cursor** | **i64** | Pass as `cursor` on the next sync. | 
**deleted** | [**Vec<uuid::Uuid>**](uuid::Uuid.md) | Tag IDs deleted at or after `cursor`. Empty on a full resync. | 
**full_resync** | **bool** | True when `changes` holds every live tag: drop cached tags it omits. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


