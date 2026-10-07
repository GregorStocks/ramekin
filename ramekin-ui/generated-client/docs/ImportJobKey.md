
# ImportJobKey

An import job submitted with an idempotency key

## Properties

Name | Type
------------ | -------------
`idempotencyKey` | string
`jobId` | string

## Example

```typescript
import type { ImportJobKey } from ''

// TODO: Update the object below with actual values
const example = {
  "idempotencyKey": null,
  "jobId": null,
} satisfies ImportJobKey

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as ImportJobKey
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


