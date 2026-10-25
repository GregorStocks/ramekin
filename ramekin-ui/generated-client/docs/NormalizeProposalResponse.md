
# NormalizeProposalResponse


## Properties

Name | Type
------------ | -------------
`base` | [NormalizedRecipe](NormalizedRecipe.md)
`baseVersionId` | string
`ops` | Array&lt;string&gt;
`proposed` | [NormalizedRecipe](NormalizedRecipe.md)
`status` | string
`summary` | string

## Example

```typescript
import type { NormalizeProposalResponse } from ''

// TODO: Update the object below with actual values
const example = {
  "base": null,
  "baseVersionId": null,
  "ops": null,
  "proposed": null,
  "status": null,
  "summary": null,
} satisfies NormalizeProposalResponse

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as NormalizeProposalResponse
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


