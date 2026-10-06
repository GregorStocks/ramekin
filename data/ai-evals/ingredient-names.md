# AI eval: Ingredient names

Written by `make ai-eval`. Picking the catalog food a name means from its candidates (`resolve_ingredient_names`), for 100 curated aliases (correct if the answer resolves to the alias's food or another entry the catalog computes identically, with the same calories, density and piece weights, since the catalog has several entries for many foods; the alias itself is not offered) and 30 curated not-food names. Some curated aliases are judgment calls (delicata squash counts as acorn squash), so a sensible answer can score as wrong: compare models with each other rather than reading this as absolute accuracy.

Golden set: `data/ai-evals/golden/ingredient-names.json` (130 cases), asked 40 per call (production asks 40). Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), or a provider error that outlasted its retries; a rejected batch is retried as the production worker does (in halves for weights, item by item for names), and a rejected single item is invalid. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Correct food | Wrong food | Unknown | Not food right | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 84% | 13% | 1% | 97% | 2% | 4 | 0 | $0.0340 |
| google/gemini-3.8-flash | 85% | 15% | 0% | 97% | 0% | 1 | 0 | $0.1679 |
| google/gemini-3.1-pro-preview | 85% | 15% | 0% | 97% | 0% | 0 | 0 | $0.3162 |
| anthropic/claude-sonnet-5.5 | 90% | 10% | 0% | 97% | 0% | 1 | 0 | $0.2035 |
| anthropic/claude-opus-5.5 | 91% | 9% | 0% | 97% | 0% | 0 | 0 | $0.2842 |
| openai/gpt-6.1-sol | 90% | 9% | 1% | 97% | 0% | 0 | 0 | $0.0745 |
| openai/gpt-6-luna | 85% | 15% | 0% | 93% | 0% | 1 | 0 | $0.0081 |

## Rejected answers

The first few items each model answered invalidly even alone.

### google/gemini-2.5-flash

- Failed to parse response: "fresh or frozen green peas": key Some("fresh or frozen green peas") is not one of its candidates
- Failed to parse response: "frozen green peas": key Some("frozen green peas") is not one of its candidates

## Worst misses

### google/gemini-2.5-flash

- flour tortillas: Entry("flour or corn tortillas") (expected tortillas, ready-to-bake or -fry, flour, refrigerated)
- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 46.0, grams_per_cup: Some(140.0), grams_per_piece: Some(300.0) }) (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- red chili powder or cayenne: Entry("chili powder or cayenne") (expected spices, pepper, red or cayenne)
- frozen corn: Entry("fresh or frozen corn") (expected corn, sweet, yellow, frozen, kernels cut off cob, unprepared (includes foods for usda's food distribution program))

### google/gemini-3.8-flash

- stock or broth: Estimate(EstimatedFood { kcal_per_100g: 10.0, grams_per_cup: Some(240.0), grams_per_piece: None }) (expected chicken broth)
- flour tortillas: Entry("tortillas, ready-to-bake or -fry, flour, shelf stable") (expected tortillas, ready-to-bake or -fry, flour, refrigerated)
- fresh or frozen green peas: Entry("frozen or fresh green peas") (expected peas, green, raw)
- delicata squash: Entry("winter squash") (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)

### google/gemini-3.1-pro-preview

- flour tortillas: Entry("tortillas, ready-to-bake or -fry, flour, shelf stable") (expected tortillas, ready-to-bake or -fry, flour, refrigerated)
- ripe avocado: Entry("avocados, raw, california") (expected avocados, raw, all commercial varieties)
- fresh or frozen green peas: Entry("peas, green, frozen, unprepared") (expected peas, green, raw)
- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 34.0, grams_per_cup: Some(116.0), grams_per_piece: Some(400.0) }) (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)

### anthropic/claude-sonnet-5.5

- fresh or frozen green peas: Entry("frozen or fresh green peas") (expected peas, green, raw)
- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 40.0, grams_per_cup: Some(130.0), grams_per_piece: Some(300.0) }) (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- red chili powder or cayenne: Entry("chili powder or cayenne") (expected spices, pepper, red or cayenne)
- aluminum-free baking powder: Entry("baking powder") (expected leavening agents, baking powder, double-acting, straight phosphate)

### anthropic/claude-opus-5.5

- fresh or frozen green peas: Entry("frozen or fresh green peas") (expected peas, green, raw)
- delicata squash: Entry("winter squash") (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- aluminum-free baking powder: Entry("baking powder") (expected leavening agents, baking powder, double-acting, straight phosphate)
- boneless beef chuck: Entry("boneless beef chuck roast") (expected beef, chuck, blade roast, separable lean and fat, trimmed to 1/8" fat, all grades, raw)

### openai/gpt-6.1-sol

- flour tortillas: Entry("tortillas, ready-to-bake or -fry, flour, shelf stable") (expected tortillas, ready-to-bake or -fry, flour, refrigerated)
- fresh or frozen green peas: Entry("frozen or fresh green peas") (expected peas, green, raw)
- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 34.0, grams_per_cup: Some(140.0), grams_per_piece: Some(500.0) }) (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- boneless beef chuck: Entry("boneless beef chuck roast") (expected beef, chuck, blade roast, separable lean and fat, trimmed to 1/8" fat, all grades, raw)

### openai/gpt-6-luna

- stock or broth: Estimate(EstimatedFood { kcal_per_100g: 5.0, grams_per_cup: Some(240.0), grams_per_piece: None }) (expected chicken broth)
- flour tortillas: Entry("flour or corn tortillas") (expected tortillas, ready-to-bake or -fry, flour, refrigerated)
- dry red wine: Entry("dry white or red wine") (expected alcoholic beverage, wine, table, red)
- fresh or frozen green peas: Entry("frozen or fresh green peas") (expected peas, green, raw)
- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 34.0, grams_per_cup: Some(140.0), grams_per_piece: Some(600.0) }) (expected squash, winter, acorn, raw)
