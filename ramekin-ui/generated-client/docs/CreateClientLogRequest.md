
# CreateClientLogRequest


## Properties

Name | Type
------------ | -------------
`appVersion` | string
`content` | string
`osInfo` | string
`platform` | string

## Example

```typescript
import type { CreateClientLogRequest } from ''

// TODO: Update the object below with actual values
const example = {
  "appVersion": null,
  "content": null,
  "osInfo": null,
  "platform": null,
} satisfies CreateClientLogRequest

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as CreateClientLogRequest
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


