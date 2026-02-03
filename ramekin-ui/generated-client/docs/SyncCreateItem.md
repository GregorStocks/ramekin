
# SyncCreateItem

Request to create an item during sync (created offline)

## Properties

Name | Type
------------ | -------------
`amount` | string
`clientId` | string
`isChecked` | boolean
`item` | string
`note` | string
`sortOrder` | number
`sourceRecipeId` | string
`sourceRecipeTitle` | string

## Example

```typescript
import type { SyncCreateItem } from ''

// TODO: Update the object below with actual values
const example = {
  "amount": null,
  "clientId": null,
  "isChecked": null,
  "item": null,
  "note": null,
  "sortOrder": null,
  "sourceRecipeId": null,
  "sourceRecipeTitle": null,
} satisfies SyncCreateItem

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as SyncCreateItem
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


