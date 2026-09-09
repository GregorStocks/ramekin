
# PrepareTextRecipeResponse


## Properties

Name | Type
------------ | -------------
`content` | [RecipeContent](RecipeContent.md)
`rawIngredients` | string
`warnings` | Array&lt;string&gt;

## Example

```typescript
import type { PrepareTextRecipeResponse } from ''

// TODO: Update the object below with actual values
const example = {
  "content": null,
  "rawIngredients": null,
  "warnings": null,
} satisfies PrepareTextRecipeResponse

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as PrepareTextRecipeResponse
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


