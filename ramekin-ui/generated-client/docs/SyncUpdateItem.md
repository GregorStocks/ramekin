
# SyncUpdateItem

Request to update an item during sync (modified offline)

## Properties

Name | Type
------------ | -------------
`amount` | string
`expectedVersion` | number
`id` | string
`isChecked` | boolean
`item` | string
`note` | string
`sortOrder` | number

## Example

```typescript
import type { SyncUpdateItem } from ''

// TODO: Update the object below with actual values
const example = {
  "amount": null,
  "expectedVersion": null,
  "id": null,
  "isChecked": null,
  "item": null,
  "note": null,
  "sortOrder": null,
} satisfies SyncUpdateItem

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as SyncUpdateItem
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


