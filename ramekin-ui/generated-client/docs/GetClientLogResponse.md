
# GetClientLogResponse


## Properties

Name | Type
------------ | -------------
`appVersion` | string
`content` | string
`createdAt` | Date
`id` | string
`osInfo` | string
`platform` | string

## Example

```typescript
import type { GetClientLogResponse } from ''

// TODO: Update the object below with actual values
const example = {
  "appVersion": null,
  "content": null,
  "createdAt": null,
  "id": null,
  "osInfo": null,
  "platform": null,
} satisfies GetClientLogResponse

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as GetClientLogResponse
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


