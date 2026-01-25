
# ListEnrichmentsResponse

Response from the list enrichments endpoint.

## Properties

Name | Type
------------ | -------------
`enrichments` | [Array&lt;EnrichmentInfo&gt;](EnrichmentInfo.md)

## Example

```typescript
import type { ListEnrichmentsResponse } from ''

// TODO: Update the object below with actual values
const example = {
  "enrichments": null,
} satisfies ListEnrichmentsResponse

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as ListEnrichmentsResponse
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


