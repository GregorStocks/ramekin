
# UpdateRecipeRequest


## Properties

Name | Type
------------ | -------------
`description` | string
`ingredients` | [Array&lt;Ingredient&gt;](Ingredient.md)
`instructions` | string
`photoIds` | Array&lt;string&gt;
`sourceName` | string
`sourceUrl` | string
`tags` | Array&lt;string&gt;
`title` | string

## Example

```typescript
import type { UpdateRecipeRequest } from ''

// TODO: Update the object below with actual values
const example = {
  "description": null,
  "ingredients": null,
  "instructions": null,
  "photoIds": null,
  "sourceName": null,
  "sourceUrl": null,
  "tags": null,
  "title": null,
} satisfies UpdateRecipeRequest

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as UpdateRecipeRequest
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


