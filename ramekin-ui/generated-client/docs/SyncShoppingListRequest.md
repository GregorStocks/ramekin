
# SyncShoppingListRequest


## Properties

Name | Type
------------ | -------------
`cursor` | number
`ops` | [Array&lt;ShoppingListOp&gt;](ShoppingListOp.md)

## Example

```typescript
import type { SyncShoppingListRequest } from ''

// TODO: Update the object below with actual values
const example = {
  "cursor": null,
  "ops": null,
} satisfies SyncShoppingListRequest

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as SyncShoppingListRequest
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


