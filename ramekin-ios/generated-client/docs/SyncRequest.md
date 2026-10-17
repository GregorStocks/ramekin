# SyncRequest

## Properties
Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**creates** | [SyncCreateItem] | Items created offline | [optional] 
**cursor** | **Int64** | &#x60;cursor&#x60; from the previous sync&#39;s response. Server returns changes at or after it; without one, it returns every item. | [optional] 
**deletes** | **[UUID]** | IDs of items deleted offline | [optional] 
**updates** | [SyncUpdateItem] | Items updated offline | [optional] 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


