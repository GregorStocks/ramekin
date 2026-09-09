
# EstimateCaloriesRequest


## Properties

Name | Type
------------ | -------------
`ingredients` | [Array&lt;Ingredient&gt;](Ingredient.md)
`scale` | number
`servings` | string

## Example

```typescript
import type { EstimateCaloriesRequest } from ''

// TODO: Update the object below with actual values
const example = {
  "ingredients": null,
  "scale": null,
  "servings": null,
} satisfies EstimateCaloriesRequest

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as EstimateCaloriesRequest
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


