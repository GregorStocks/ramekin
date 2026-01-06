
# RecipeResponse


## Properties

Name | Type
------------ | -------------
`createdAt` | Date
`description` | string
`id` | string
`ingredients` | [Array&lt;Ingredient&gt;](Ingredient.md)
`instructions` | string
`photoIds` | Array&lt;string&gt;
`sourceName` | string
`sourceUrl` | string
`tags` | Array&lt;string&gt;
`title` | string
`updatedAt` | Date

## Example

```typescript
import type { RecipeResponse } from ''

// TODO: Update the object below with actual values
const example = {
  "createdAt": null,
  "description": null,
  "id": null,
  "ingredients": null,
  "instructions": null,
  "photoIds": null,
  "sourceName": null,
  "sourceUrl": null,
  "tags": null,
  "title": null,
  "updatedAt": null,
} satisfies RecipeResponse

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as RecipeResponse
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


