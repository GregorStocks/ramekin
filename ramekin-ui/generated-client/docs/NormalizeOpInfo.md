
# NormalizeOpInfo

One normalize op, as the registry defines it.

## Properties

Name | Type
------------ | -------------
`available` | boolean
`defaultOnImport` | boolean
`group` | [NormalizeOpGroupResponse](NormalizeOpGroupResponse.md)
`id` | string
`implies` | Array&lt;string&gt;
`label` | string
`runsAfter` | Array&lt;string&gt;
`unavailableReason` | string
`usesAi` | boolean
`writes` | Array&lt;string&gt;

## Example

```typescript
import type { NormalizeOpInfo } from ''

// TODO: Update the object below with actual values
const example = {
  "available": null,
  "defaultOnImport": null,
  "group": null,
  "id": null,
  "implies": null,
  "label": null,
  "runsAfter": null,
  "unavailableReason": null,
  "usesAi": null,
  "writes": null,
} satisfies NormalizeOpInfo

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as NormalizeOpInfo
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


