
# SyncMealPlansRequest


## Properties

Name | Type
------------ | -------------
`cursor` | number
`ops` | [Array&lt;MealPlanOp&gt;](MealPlanOp.md)

## Example

```typescript
import type { SyncMealPlansRequest } from ''

// TODO: Update the object below with actual values
const example = {
  "cursor": null,
  "ops": null,
} satisfies SyncMealPlansRequest

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as SyncMealPlansRequest
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


