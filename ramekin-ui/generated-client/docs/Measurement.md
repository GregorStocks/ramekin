
# Measurement

A single measurement (amount + unit pair)

## Properties

Name | Type
------------ | -------------
`amount` | string
`unit` | string

## Example

```typescript
import type { Measurement } from ''

// TODO: Update the object below with actual values
const example = {
  "amount": null,
  "unit": null,
} satisfies Measurement

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Measurement
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


