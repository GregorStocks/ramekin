
# RecipeResponse


## Properties

Name | Type
------------ | -------------
`cookTime` | string
`createdAt` | Date
`description` | string
`difficulty` | string
`id` | string
`ingredients` | [Array&lt;Ingredient&gt;](Ingredient.md)
`instructions` | string
`notes` | string
`nutritionalInfo` | string
`photoIds` | Array&lt;string&gt;
`prepTime` | string
`rating` | number
`servings` | string
`sourceName` | string
`sourceUrl` | string
`tags` | Array&lt;string&gt;
`title` | string
`totalTime` | string
`updatedAt` | Date

## Example

```typescript
import type { RecipeResponse } from ''

// TODO: Update the object below with actual values
const example = {
  "cookTime": null,
  "createdAt": null,
  "description": null,
  "difficulty": null,
  "id": null,
  "ingredients": null,
  "instructions": null,
  "notes": null,
  "nutritionalInfo": null,
  "photoIds": null,
  "prepTime": null,
  "rating": null,
  "servings": null,
  "sourceName": null,
  "sourceUrl": null,
  "tags": null,
  "title": null,
  "totalTime": null,
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


