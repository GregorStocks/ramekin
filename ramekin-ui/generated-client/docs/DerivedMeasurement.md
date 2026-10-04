
# DerivedMeasurement

Approximate grams for one ingredient line: oz/lb converted, or a volume through the catalog\'s density for the food. Computed on every read and never stored, so catalog improvements reach every recipe; never send these back when saving.

## Properties

Name | Type
------------ | -------------
`amount` | string
`ingredientIndex` | number
`unit` | string

## Example

```typescript
import type { DerivedMeasurement } from ''

// TODO: Update the object below with actual values
const example = {
  "amount": null,
  "ingredientIndex": null,
  "unit": null,
} satisfies DerivedMeasurement

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as DerivedMeasurement
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


