# ReparseIngredientsResponse

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**recipes_checked** | **i32** | Live recipes looked at. | 
**recipes_updated** | **i32** | Recipes that got a new \"reparse\" version because their ingredients changed. | 
**skipped_edited** | **i32** | Recipes edited by hand since their last scrape, or changed while the re-parse ran. | 
**skipped_no_source** | **i32** | Recipes with no saved scrape to re-read: typed in, or from text or a photo, or a scrape whose page or text can't be read. | 

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


