
# ShoppingGroup

A recipe\'s ingredients that share a shopping-list category.

## Properties

Name | Type
------------ | -------------
`category` | string
`ingredientIndices` | Array&lt;number&gt;

## Example

```typescript
import type { ShoppingGroup } from ''

// TODO: Update the object below with actual values
const example = {
  "category": null,
  "ingredientIndices": null,
} satisfies ShoppingGroup

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as ShoppingGroup
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


