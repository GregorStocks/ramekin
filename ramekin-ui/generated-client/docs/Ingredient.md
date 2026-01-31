
# Ingredient

Ingredient structure for JSONB storage

## Properties

Name | Type
------------ | -------------
`item` | string
`measurements` | [Array&lt;Measurement&gt;](Measurement.md)
`note` | string
`raw` | string
`section` | string

## Example

```typescript
import type { Ingredient } from ''

// TODO: Update the object below with actual values
const example = {
  "item": null,
  "measurements": null,
  "note": null,
  "raw": null,
  "section": null,
} satisfies Ingredient

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Ingredient
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


