
# ImportRawRecipe

Raw recipe data for import (mirrors ramekin_core::RawRecipe)

## Properties

Name | Type
------------ | -------------
`categories` | Array&lt;string&gt;
`cookTime` | string
`description` | string
`difficulty` | string
`imageUrls` | Array&lt;string&gt;
`ingredients` | string
`instructions` | string
`notes` | string
`nutritionalInfo` | string
`prepTime` | string
`rating` | number
`servings` | string
`sourceName` | string
`sourceUrl` | string
`title` | string
`totalTime` | string

## Example

```typescript
import type { ImportRawRecipe } from ''

// TODO: Update the object below with actual values
const example = {
  "categories": null,
  "cookTime": null,
  "description": null,
  "difficulty": null,
  "imageUrls": null,
  "ingredients": null,
  "instructions": null,
  "notes": null,
  "nutritionalInfo": null,
  "prepTime": null,
  "rating": null,
  "servings": null,
  "sourceName": null,
  "sourceUrl": null,
  "title": null,
  "totalTime": null,
} satisfies ImportRawRecipe

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as ImportRawRecipe
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


