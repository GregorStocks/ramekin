
# CalorieEstimateResponse


## Properties

Name | Type
------------ | -------------
`databaseVersion` | string
`knownCalories` | [CalorieRange](CalorieRange.md)
`perServingCalories` | [CalorieRange](CalorieRange.md)
`perServingSummary` | string
`summary` | string
`unknownIngredients` | [Array&lt;UnknownCalorieIngredient&gt;](UnknownCalorieIngredient.md)

## Example

```typescript
import type { CalorieEstimateResponse } from ''

// TODO: Update the object below with actual values
const example = {
  "databaseVersion": null,
  "knownCalories": null,
  "perServingCalories": null,
  "perServingSummary": null,
  "summary": null,
  "unknownIngredients": null,
} satisfies CalorieEstimateResponse

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as CalorieEstimateResponse
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


