
# NormalizedRecipe

A recipe\'s content and photos: what a normalize run starts from, and what it proposes.

## Properties

Name | Type
------------ | -------------
`content` | [RecipeContent](RecipeContent.md)
`photoIds` | Array&lt;string&gt;

## Example

```typescript
import type { NormalizedRecipe } from ''

// TODO: Update the object below with actual values
const example = {
  "content": null,
  "photoIds": null,
} satisfies NormalizedRecipe

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as NormalizedRecipe
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


