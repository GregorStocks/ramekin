
# SyncRecipe

Read-only recipe data needed to populate the iOS cache and mirror server search.

## Properties

Name | Type
------------ | -------------
`createdAt` | Date
`description` | string
`id` | string
`ingredients` | [Array&lt;Ingredient&gt;](Ingredient.md)
`instructions` | string
`notes` | string
`rating` | number
`tags` | Array&lt;string&gt;
`thumbnailPhotoId` | string
`title` | string
`updatedAt` | Date

## Example

```typescript
import type { SyncRecipe } from ''

// TODO: Update the object below with actual values
const example = {
  "createdAt": null,
  "description": null,
  "id": null,
  "ingredients": null,
  "instructions": null,
  "notes": null,
  "rating": null,
  "tags": null,
  "thumbnailPhotoId": null,
  "title": null,
  "updatedAt": null,
} satisfies SyncRecipe

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as SyncRecipe
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


