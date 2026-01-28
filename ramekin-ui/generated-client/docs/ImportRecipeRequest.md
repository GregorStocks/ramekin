
# ImportRecipeRequest

Request body for importing a recipe

## Properties

Name | Type
------------ | -------------
`extractionMethod` | [ImportExtractionMethod](ImportExtractionMethod.md)
`photoIds` | Array&lt;string&gt;
`rawRecipe` | [ImportRawRecipe](ImportRawRecipe.md)

## Example

```typescript
import type { ImportRecipeRequest } from ''

// TODO: Update the object below with actual values
const example = {
  "extractionMethod": null,
  "photoIds": null,
  "rawRecipe": null,
} satisfies ImportRecipeRequest

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as ImportRecipeRequest
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


