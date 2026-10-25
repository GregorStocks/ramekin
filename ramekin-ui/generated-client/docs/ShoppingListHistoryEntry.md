
# ShoppingListHistoryEntry


## Properties

Name | Type
------------ | -------------
`createdAt` | Date
`id` | string
`summary` | string
`undoesId` | string
`undoneById` | string

## Example

```typescript
import type { ShoppingListHistoryEntry } from ''

// TODO: Update the object below with actual values
const example = {
  "createdAt": null,
  "id": null,
  "summary": null,
  "undoesId": null,
  "undoneById": null,
} satisfies ShoppingListHistoryEntry

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as ShoppingListHistoryEntry
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


