
# ScrapeJobResponse


## Properties

Name | Type
------------ | -------------
`canRetry` | boolean
`error` | string
`failedAtStep` | string
`id` | string
`recipeId` | string
`retryCount` | number
`status` | string
`url` | string

## Example

```typescript
import type { ScrapeJobResponse } from ''

// TODO: Update the object below with actual values
const example = {
  "canRetry": null,
  "error": null,
  "failedAtStep": null,
  "id": null,
  "recipeId": null,
  "retryCount": null,
  "status": null,
  "url": null,
} satisfies ScrapeJobResponse

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as ScrapeJobResponse
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


