
# MealPlanFields

A meal plan\'s writable fields. On `create`, `recipe_id`, `meal_date` and `meal_type` are required. On `update`, only the fields present change, and `recipe_id` can\'t. `notes` is cleared with \"\".

## Properties

Name | Type
------------ | -------------
`mealDate` | Date
`mealType` | [MealType](MealType.md)
`notes` | string
`recipeId` | string

## Example

```typescript
import type { MealPlanFields } from ''

// TODO: Update the object below with actual values
const example = {
  "mealDate": null,
  "mealType": null,
  "notes": null,
  "recipeId": null,
} satisfies MealPlanFields

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as MealPlanFields
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


