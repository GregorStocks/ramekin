
# MealPlanItem


## Properties

Name | Type
------------ | -------------
`id` | string
`mealDate` | Date
`mealType` | [MealType](MealType.md)
`notes` | string
`recipeId` | string
`recipeTitle` | string
`thumbnailPhotoId` | string

## Example

```typescript
import type { MealPlanItem } from ''

// TODO: Update the object below with actual values
const example = {
  "id": null,
  "mealDate": null,
  "mealType": null,
  "notes": null,
  "recipeId": null,
  "recipeTitle": null,
  "thumbnailPhotoId": null,
} satisfies MealPlanItem

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as MealPlanItem
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


