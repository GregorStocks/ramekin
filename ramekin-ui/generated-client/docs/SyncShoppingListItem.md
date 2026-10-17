
# SyncShoppingListItem


## Properties

Name | Type
------------ | -------------
`amount` | string
`category` | string
`categoryOverride` | string
`changeSeq` | number
`computedCategory` | string
`id` | string
`isChecked` | boolean
`item` | string
`note` | string
`sortOrder` | number
`sourceRecipeId` | string
`sourceRecipeTitle` | string

## Example

```typescript
import type { SyncShoppingListItem } from ''

// TODO: Update the object below with actual values
const example = {
  "amount": null,
  "category": null,
  "categoryOverride": null,
  "changeSeq": null,
  "computedCategory": null,
  "id": null,
  "isChecked": null,
  "item": null,
  "note": null,
  "sortOrder": null,
  "sourceRecipeId": null,
  "sourceRecipeTitle": null,
} satisfies SyncShoppingListItem

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as SyncShoppingListItem
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


