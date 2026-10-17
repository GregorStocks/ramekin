
# ShoppingListFields

An item\'s writable fields. On `create`, `item` is required and the rest default (unchecked, sorted last). On `update`, only the fields present change. `amount`, `note` and `category_override` are cleared with \"\". The source recipe is set on create and never changes.

## Properties

Name | Type
------------ | -------------
`amount` | string
`categoryOverride` | string
`isChecked` | boolean
`item` | string
`note` | string
`sortOrder` | number
`sourceRecipeId` | string
`sourceRecipeTitle` | string

## Example

```typescript
import type { ShoppingListFields } from ''

// TODO: Update the object below with actual values
const example = {
  "amount": null,
  "categoryOverride": null,
  "isChecked": null,
  "item": null,
  "note": null,
  "sortOrder": null,
  "sourceRecipeId": null,
  "sourceRecipeTitle": null,
} satisfies ShoppingListFields

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as ShoppingListFields
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


