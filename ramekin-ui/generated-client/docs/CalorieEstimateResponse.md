
# CalorieEstimateResponse

Every display string is final; clients render them as-is.

## Properties

Name | Type
------------ | -------------
`databaseVersion` | string
`headline` | string
`knownCalories` | [CalorieRange](CalorieRange.md)
`lines` | [Array&lt;CalorieLine&gt;](CalorieLine.md)
`notCounted` | Array&lt;string&gt;
`perServingCalories` | [CalorieRange](CalorieRange.md)
`secondary` | string
`status` | [CalorieStatus](CalorieStatus.md)

## Example

```typescript
import type { CalorieEstimateResponse } from ''

// TODO: Update the object below with actual values
const example = {
  "databaseVersion": null,
  "headline": null,
  "knownCalories": null,
  "lines": null,
  "notCounted": null,
  "perServingCalories": null,
  "secondary": null,
  "status": null,
} satisfies CalorieEstimateResponse

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as CalorieEstimateResponse
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


