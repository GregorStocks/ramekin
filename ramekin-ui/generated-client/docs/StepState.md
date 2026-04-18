
# StepState

A single pipeline step\'s state for the status API response.

## Properties

Name | Type
------------ | -------------
`durationMs` | number
`error` | string
`finishedAt` | Date
`hasOutput` | boolean
`name` | string
`startedAt` | Date
`status` | string
`summary` | string

## Example

```typescript
import type { StepState } from ''

// TODO: Update the object below with actual values
const example = {
  "durationMs": null,
  "error": null,
  "finishedAt": null,
  "hasOutput": null,
  "name": null,
  "startedAt": null,
  "status": null,
  "summary": null,
} satisfies StepState

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as StepState
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


