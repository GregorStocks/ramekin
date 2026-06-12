
# ErrorCode

Machine-readable error code surfaced in every error response.  Clients branch on this instead of parsing the human-readable `error` message or guessing from the HTTP status, so changing the wording of a message is never a contract change. Each code maps to exactly one HTTP status (see [`ErrorCode::status`]).

## Properties

Name | Type
------------ | -------------

## Example

```typescript
import type { ErrorCode } from ''

// TODO: Update the object below with actual values
const example = {
} satisfies ErrorCode

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as ErrorCode
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


