# Ingredient

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**item** | **String** | The ingredient name (e.g., \"butter\", \"all-purpose flour\") | 
**measurements** | [**Vec<models::Measurement>**](Measurement.md) | Measurements - first is primary, rest are alternatives (e.g., \"1 stick\" then \"113g\") | 
**note** | Option<**String**> | Preparation notes (e.g., \"chopped\", \"softened\", \"optional\") | [optional]
**raw** | Option<**String**> | Original unparsed text for debugging | [optional]
**section** | Option<**String**> | Section name for grouping (e.g., \"For the sauce\", \"For the dough\") | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


