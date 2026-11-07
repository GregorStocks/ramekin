
# IngredientNamesStall

The background resolver\'s last pass failed for a reason that isn\'t any one name\'s (the provider is down, rate-limited, or misconfigured), so pending names wait for it to try again.

## Properties

Name | Type
------------ | -------------
`error` | string
`nextAttemptAt` | Date

## Example

```typescript
import type { IngredientNamesStall } from ''

// TODO: Update the object below with actual values
const example = {
  "error": null,
  "nextAttemptAt": null,
} satisfies IngredientNamesStall

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as IngredientNamesStall
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


