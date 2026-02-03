
# UpdateShoppingListItemRequest


## Properties

Name | Type
------------ | -------------
`amount` | string
`isChecked` | boolean
`item` | string
`note` | string
`sortOrder` | number

## Example

```typescript
import type { UpdateShoppingListItemRequest } from ''

// TODO: Update the object below with actual values
const example = {
  "amount": null,
  "isChecked": null,
  "item": null,
  "note": null,
  "sortOrder": null,
} satisfies UpdateShoppingListItemRequest

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as UpdateShoppingListItemRequest
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


