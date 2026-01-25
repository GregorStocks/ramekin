
# RecipeContent

Core recipe content - all fields that can be enriched by AI. Used for enrichment APIs and recipe import.

## Properties

Name | Type
------------ | -------------
`cookTime` | string
`description` | string
`difficulty` | string
`ingredients` | [Array&lt;Ingredient&gt;](Ingredient.md)
`instructions` | string
`notes` | string
`nutritionalInfo` | string
`prepTime` | string
`rating` | number
`servings` | string
`sourceName` | string
`sourceUrl` | string
`tags` | Array&lt;string&gt;
`title` | string
`totalTime` | string

## Example

```typescript
import type { RecipeContent } from ''

// TODO: Update the object below with actual values
const example = {
  "cookTime": null,
  "description": null,
  "difficulty": null,
  "ingredients": null,
  "instructions": null,
  "notes": null,
  "nutritionalInfo": null,
  "prepTime": null,
  "rating": null,
  "servings": null,
  "sourceName": null,
  "sourceUrl": null,
  "tags": null,
  "title": null,
  "totalTime": null,
} satisfies RecipeContent

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as RecipeContent
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


