
# IngredientNamesStatusResponse


## Properties

Name | Type
------------ | -------------
`failed` | number
`failures` | [Array&lt;IngredientNameFailure&gt;](IngredientNameFailure.md)
`notFood` | number
`pending` | number
`recognized` | number
`unknown` | number
`weights` | [IngredientWeightsStatus](IngredientWeightsStatus.md)

## Example

```typescript
import type { IngredientNamesStatusResponse } from ''

// TODO: Update the object below with actual values
const example = {
  "failed": null,
  "failures": null,
  "notFood": null,
  "pending": null,
  "recognized": null,
  "unknown": null,
  "weights": null,
} satisfies IngredientNamesStatusResponse

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as IngredientNamesStatusResponse
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


