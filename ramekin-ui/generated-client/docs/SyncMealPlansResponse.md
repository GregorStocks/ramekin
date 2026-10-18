
# SyncMealPlansResponse


## Properties

Name | Type
------------ | -------------
`changes` | [Array&lt;SyncMealPlan&gt;](SyncMealPlan.md)
`cursor` | number
`deleted` | Array&lt;string&gt;
`fullResync` | boolean
`results` | [Array&lt;OpResult&gt;](OpResult.md)

## Example

```typescript
import type { SyncMealPlansResponse } from ''

// TODO: Update the object below with actual values
const example = {
  "changes": null,
  "cursor": null,
  "deleted": null,
  "fullResync": null,
  "results": null,
} satisfies SyncMealPlansResponse

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as SyncMealPlansResponse
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


