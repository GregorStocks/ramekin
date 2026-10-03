# AI eval: Ingredient names

Written by `make ai-eval`. Picking the catalog food a name means from its candidates (`resolve_ingredient_names`), for 100 curated aliases (correct if the answer resolves to the alias's food; the alias itself is not offered) and 30 curated not-food names. Some curated aliases are judgment calls (delicata squash counts as acorn squash), so a sensible answer can score as wrong: compare models with each other rather than reading this as absolute accuracy.

Golden set: `data/ai-evals/golden/ingredient-names.json` (130 cases), asked 1 per call (production asks 40). Truncated calls ran out of the production max_tokens (the batch is then retried item by item here; in production it fails). Cost is what this suite's calls cost at OpenRouter's current prices; a cached rerun spends nothing.

| Model | Correct food | Wrong food | Unknown | Not food right | Invalid | Truncated calls | Cost |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 82% | 12% | 3% | 53% | 2% | 0 | $0.0396 |
| google/gemini-3.8-flash | 87% | 13% | 0% | 80% | 0% | 0 | $0.3215 |
| openai/gpt-6.1-sol | 88% | 11% | 1% | 57% | 0% | 0 | $0.2083 |
| google/gemini-3.1-pro-preview | 84% | 16% | 0% | 77% | 0% | 0 | $1.3895 |

## Rejected answers

The first few items each model answered invalidly even alone.

### google/gemini-2.5-flash

- Failed to parse response: "fresh or frozen green peas": key Some("fresh or frozen green peas") is not one of its candidates
- Failed to parse response: "frozen green peas": key Some("frozen green peas") is not one of its candidates
- Failed to parse response: "fine sea or table salt": key Some("fine sea or table salt") is not one of its candidates

## Worst misses

### google/gemini-2.5-flash

- flour tortillas: Entry("tortillas, ready-to-bake or -fry, flour, shelf stable") (expected tortillas, ready-to-bake or -fry, flour, refrigerated)
- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 47.0, grams_per_cup: Some(140.0), grams_per_piece: Some(300.0) }) (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- red chili powder or cayenne: Entry("chili powder or cayenne") (expected spices, pepper, red or cayenne)
- frozen corn: Entry("fresh or frozen corn") (expected corn, sweet, yellow, frozen, kernels cut off cob, unprepared (includes foods for usda's food distribution program))

### google/gemini-3.8-flash

- stock or broth: Estimate(EstimatedFood { kcal_per_100g: 10.0, grams_per_cup: Some(240.0), grams_per_piece: None }) (expected chicken broth)
- fresh or frozen green peas: Entry("frozen or fresh green peas") (expected peas, green, raw)
- delicata squash: Entry("winter squash") (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- red chili powder or cayenne: Entry("chili powder or cayenne") (expected spices, pepper, red or cayenne)

### openai/gpt-6.1-sol

- flour tortillas: Entry("flour tortillas, warmed") (expected tortillas, ready-to-bake or -fry, flour, refrigerated)
- fresh or frozen green peas: Entry("frozen or fresh green peas") (expected peas, green, raw)
- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 40.0, grams_per_cup: Some(140.0), grams_per_piece: Some(450.0) }) (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- red chili powder or cayenne: Entry("chili powder or cayenne") (expected spices, pepper, red or cayenne)

### google/gemini-3.1-pro-preview

- stock or broth: Estimate(EstimatedFood { kcal_per_100g: 15.0, grams_per_cup: Some(240.0), grams_per_piece: None }) (expected chicken broth)
- flour tortillas: Entry("tortillas, ready-to-bake or -fry, flour, shelf stable") (expected tortillas, ready-to-bake or -fry, flour, refrigerated)
- fresh or frozen green peas: Entry("frozen or fresh green peas") (expected peas, green, raw)
- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 34.0, grams_per_cup: Some(140.0), grams_per_piece: Some(450.0) }) (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
