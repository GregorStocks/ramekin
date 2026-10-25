
# EditRecipeRequest

Request body for Edit with AI.

## Properties

Name | Type
------------ | -------------
`instruction` | string
`photoIds` | Array&lt;string&gt;

## Example

```typescript
import type { EditRecipeRequest } from ''

// TODO: Update the object below with actual values
const example = {
  "instruction": null,
  "photoIds": null,
} satisfies EditRecipeRequest

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as EditRecipeRequest
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


