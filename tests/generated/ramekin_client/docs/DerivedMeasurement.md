# DerivedMeasurement

Approximate grams for one ingredient line: oz/lb converted, or a volume through the catalog's density for the food. Computed on every read and never stored, so catalog improvements reach every recipe; never send these back when saving.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**amount** | **str** |  | 
**ingredient_index** | **int** | Index of the ingredient this belongs to. | 
**unit** | **str** |  | 

## Example

```python
from ramekin_client.models.derived_measurement import DerivedMeasurement

# TODO update the JSON string below
json = "{}"
# create an instance of DerivedMeasurement from a JSON string
derived_measurement_instance = DerivedMeasurement.from_json(json)
# print the JSON string representation of the object
print(DerivedMeasurement.to_json())

# convert the object into a dict
derived_measurement_dict = derived_measurement_instance.to_dict()
# create an instance of DerivedMeasurement from a dict
derived_measurement_from_dict = DerivedMeasurement.from_dict(derived_measurement_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


