
# SyncRecipesResponse


## Properties

Name | Type
------------ | -------------
`deleted` | Array&lt;string&gt;
`recipes` | [Array&lt;SyncRecipe&gt;](SyncRecipe.md)
`syncTimestamp` | Date

## Example

```typescript
import type { SyncRecipesResponse } from ''

// TODO: Update the object below with actual values
const example = {
  "deleted": null,
  "recipes": null,
  "syncTimestamp": null,
} satisfies SyncRecipesResponse

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as SyncRecipesResponse
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


