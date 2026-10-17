
# SyncResponse


## Properties

Name | Type
------------ | -------------
`categoryOrder` | Array&lt;string&gt;
`created` | [Array&lt;SyncCreatedItem&gt;](SyncCreatedItem.md)
`cursor` | number
`deleted` | Array&lt;string&gt;
`fullResync` | boolean
`serverChanges` | [Array&lt;SyncServerChange&gt;](SyncServerChange.md)
`updated` | [Array&lt;SyncUpdatedItem&gt;](SyncUpdatedItem.md)

## Example

```typescript
import type { SyncResponse } from ''

// TODO: Update the object below with actual values
const example = {
  "categoryOrder": null,
  "created": null,
  "cursor": null,
  "deleted": null,
  "fullResync": null,
  "serverChanges": null,
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


