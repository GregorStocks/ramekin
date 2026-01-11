
# VersionListResponse

Response for version list endpoint

## Properties

Name | Type
------------ | -------------
`versions` | [Array&lt;VersionSummary&gt;](VersionSummary.md)

## Example

```typescript
import type { VersionListResponse } from ''

// TODO: Update the object below with actual values
const example = {
  "versions": null,
} satisfies VersionListResponse

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as VersionListResponse
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


