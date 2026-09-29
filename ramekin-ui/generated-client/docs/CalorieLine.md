
# CalorieLine

One ingredient line\'s part of the estimate.

## Properties

Name | Type
------------ | -------------
`calories` | [CalorieRange](CalorieRange.md)
`index` | number
`item` | string
`text` | string

## Example

```typescript
import type { CalorieLine } from ''

// TODO: Update the object below with actual values
const example = {
  "calories": null,
  "index": null,
  "item": null,
  "text": null,
} satisfies CalorieLine

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as CalorieLine
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


