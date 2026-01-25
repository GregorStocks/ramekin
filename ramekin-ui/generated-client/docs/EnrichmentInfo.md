
# EnrichmentInfo

Information about an enrichment type for the API.

## Properties

Name | Type
------------ | -------------
`description` | string
`displayName` | string
`outputFields` | Array&lt;string&gt;
`type` | string

## Example

```typescript
import type { EnrichmentInfo } from ''

// TODO: Update the object below with actual values
const example = {
  "description": null,
  "displayName": null,
  "outputFields": null,
  "type": null,
} satisfies EnrichmentInfo

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as EnrichmentInfo
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


