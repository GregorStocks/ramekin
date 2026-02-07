
# RecipeSummary


## Properties

Name | Type
------------ | -------------
`createdAt` | Date
`description` | string
`id` | string
`rating` | number
`tags` | Array&lt;string&gt;
`thumbnailPhotoId` | string
`title` | string
`updatedAt` | Date

## Example

```typescript
import type { RecipeSummary } from ''

// TODO: Update the object below with actual values
const example = {
  "createdAt": null,
  "description": null,
  "id": null,
  "rating": null,
  "tags": null,
  "thumbnailPhotoId": null,
  "title": null,
  "updatedAt": null,
} satisfies RecipeSummary

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as RecipeSummary
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


