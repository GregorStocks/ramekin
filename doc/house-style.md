# House style

How a recipe reads once the `house_style` normalize op has rewritten it. The
op's prompt embeds this file as it stands
(`ramekin-core/src/ai/prompts/house_style.rs`), so changing it changes the op;
rerun `make ai-eval SUITE=house-style` afterwards.

The voice is NYT Cooking's: plain, imperative, specific. Grams are not part of
the rewrite: the server derives them from the catalog on read, so the op never
changes an amount or a unit.

## What never changes

- Amounts and units. "1 cup flour" stays 1 cup; don't convert, round or add
  weights.
- Which foods the recipe uses. Don't add, drop, merge or substitute
  ingredients, even an obvious garnish the steps forgot.
- What the cook does. Rewording, splitting and adding prep steps are fine;
  changing a temperature, a time, an order of operations or a technique isn't.
- Title, description, notes, servings and times.

## Ingredient lines

- Amount, unit, then the ingredient: "2 tbsp olive oil". Short units: g, kg,
  ml, l, tsp, tbsp, cup, oz, lb. Keep the line's amounts exactly; only the
  spelling of the unit changes ("2 Tablespoons" becomes "2 tbsp").
- Lowercase ingredient names, keeping proper nouns: "Dijon mustard", "Parmesan",
  "San Marzano tomatoes".
- No prep in the line. Cutting, chopping, mincing, grating, peeling, zesting,
  juicing, melting, softening, toasting, beating and so on move into the steps.
  - "1 onion, finely chopped" becomes "1 onion", plus "Finely chop the onion."
    in the step that first needs it (or just before it).
  - "1 cup chopped walnuts, toasted" becomes "1 cup walnuts" and the steps say
    to toast and chop them.
  - A form you buy stays: "1 can diced tomatoes", "2 cups shredded mozzarella"
    when the recipe means the bag, "ground cumin", "frozen peas".
  - A measure that depends on the prep stays as it is, since moving it would
    change the amount: "1 cup chopped parsley" is a cup after chopping.
    Leave such a line alone.
- Descriptions that help shopping stay in the line: "1 large egg",
  "4 bone-in, skin-on chicken thighs", "1 lb ground beef (80/20)".
- "To taste" and "for serving" stay: "kosher salt, to taste".

## Ingredient order and sections

- List ingredients in the order the steps first use them.
- When a recipe has distinct components (a sauce, a dough, a filling, a
  topping), put each component's ingredients in a section, named "For the
  sauce", "For the dough" and so on, in the order the steps make them. Keep
  the source's sections when it has them, renamed to this form.
- An ingredient two components use is listed in each, with the amount each
  uses, only when the source already splits it; otherwise list it once, in the
  first component that uses it.
- A simple recipe (one component) has no sections.

## Steps

- One coherent stage per step: "Heat the oil in a large skillet over medium
  heat. Add the onion and cook, stirring, until soft, about 5 minutes." Don't
  split a stage into one action per step, and don't run several stages
  together.
- Imperative, second person implied: "Whisk the eggs", not "The eggs are
  whisked" or "You'll want to whisk".
- Prep moved out of the ingredient lines becomes its own step, or opens the
  step that first needs it, at the point it's needed. Prep for several
  ingredients at once can share a step: "Chop the onion and mince the garlic."
- Where a step isn't obvious, add a short reason: "Rest the dough for 30
  minutes so the flour hydrates." Keep it to a clause; most steps need none.
- Keep the source's doneness cues and times as they are; don't invent new
  ones.
- Temperatures in °F only: "425°F". Convert a °C-only recipe to °F, rounding
  to the nearest 5°F, and drop the °C from dual temperatures.
- No numbering or "Step 1:" labels in the text; the app numbers steps.

## A worked example

Before:

```
Ingredients:
  1. 2 Tablespoons Olive Oil
  2. 1 Yellow Onion, diced
  3. 3 cloves garlic, minced
  4. 1 (28 oz) can crushed tomatoes
  5. 1/2 cup heavy cream
  6. 1 lb rigatoni
  7. Salt to taste

Steps:
  1. Cook the pasta according to package directions.
  2. Heat the oil in a pan and saute the onion and garlic until soft. Add the tomatoes and simmer 15 minutes. Stir in cream.
  3. Toss with pasta and season.
```

After:

```
Ingredients:
  1. 1 lb rigatoni
  2. salt, to taste
  3. 2 tbsp olive oil
  4. 1 yellow onion
  5. 3 cloves garlic
  6. 1 (28 oz) can crushed tomatoes
  7. 1/2 cup heavy cream

Steps:
  1. Bring a large pot of salted water to a boil and cook the rigatoni according to the package directions.
  2. Dice the onion and mince the garlic.
  3. Heat the oil in a large pan over medium heat. Add the onion and garlic and cook until soft.
  4. Add the tomatoes and simmer for 15 minutes so the sauce thickens, then stir in the cream.
  5. Toss the sauce with the drained pasta and season with salt.
```
