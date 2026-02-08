
# CustomEnrichRequest

Request body for custom enrichment.

## Properties

Name | Type
------------ | -------------
`instruction` | string
`recipe` | [RecipeContent](RecipeContent.md)

## Example

```typescript
import type { CustomEnrichRequest } from ''

// TODO: Update the object below with actual values
const example = {
  "instruction": null,
  "recipe": null,
} satisfies CustomEnrichRequest

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as CustomEnrichRequest
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


