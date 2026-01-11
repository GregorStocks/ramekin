
# VersionSummary

Version summary for listing version history

## Properties

Name | Type
------------ | -------------
`createdAt` | Date
`id` | string
`isCurrent` | boolean
`title` | string
`versionSource` | string

## Example

```typescript
import type { VersionSummary } from ''

// TODO: Update the object below with actual values
const example = {
  "createdAt": null,
  "id": null,
  "isCurrent": null,
  "title": null,
  "versionSource": null,
} satisfies VersionSummary

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as VersionSummary
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


