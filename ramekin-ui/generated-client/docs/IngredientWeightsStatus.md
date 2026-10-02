
# IngredientWeightsStatus

Weights the catalog lacks for foods in the user\'s recipes (a density, or a counted unit such as \"bunch\"), estimated in the background.

## Properties

Name | Type
------------ | -------------
`estimated` | number
`failed` | number
`failures` | [Array&lt;IngredientWeightFailure&gt;](IngredientWeightFailure.md)
`noTypicalWeight` | number
`pending` | number

## Example

```typescript
import type { IngredientWeightsStatus } from ''

// TODO: Update the object below with actual values
const example = {
  "estimated": null,
  "failed": null,
  "failures": null,
  "noTypicalWeight": null,
  "pending": null,
} satisfies IngredientWeightsStatus

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as IngredientWeightsStatus
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


