
# SyncResponse


## Properties

Name | Type
------------ | -------------
`created` | [Array&lt;SyncCreatedItem&gt;](SyncCreatedItem.md)
`deleted` | Array&lt;string&gt;
`serverChanges` | [Array&lt;SyncServerChange&gt;](SyncServerChange.md)
`syncTimestamp` | Date
`updated` | [Array&lt;SyncUpdatedItem&gt;](SyncUpdatedItem.md)

## Example

```typescript
import type { SyncResponse } from ''

// TODO: Update the object below with actual values
const example = {
  "created": null,
  "deleted": null,
  "serverChanges": null,
  "syncTimestamp": null,
  "updated": null,
} satisfies SyncResponse

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as SyncResponse
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


