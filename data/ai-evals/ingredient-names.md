# AI eval: Ingredient names

Written by `make ai-eval`. Picking the catalog food a name means from its candidates (`resolve_ingredient_names`), for 100 curated aliases (correct if the answer resolves to the alias's food; the alias itself is not offered) and 30 curated not-food names. Some curated aliases are judgment calls (delicata squash counts as acorn squash), so a sensible answer can score as wrong: compare models with each other rather than reading this as absolute accuracy.

Golden set: `data/ai-evals/golden/ingredient-names.json` (130 cases), asked 40 per call (production asks 40). Truncated calls ran out of the production max_tokens (the batch is then retried item by item here; in production it fails). Cost is what this suite's calls cost at OpenRouter's current prices; a cached rerun spends nothing.

| Model | Correct food | Wrong food | Unknown | Not food right | Invalid | Truncated calls | Cost |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 81% | 17% | 1% | 57% | 1% | 0 | $0.0242 |
| google/gemini-3.8-flash | 86% | 14% | 0% | 83% | 0% | 0 | $0.1856 |
| google/gemini-3.1-pro-preview | 87% | 13% | 0% | 87% | 0% | 0 | $0.3260 |
| anthropic/claude-sonnet-5.5 | 68% | 7% | 1% | 80% | 18% | 0 | $0.1085 |
| anthropic/claude-opus-5.5 | 75% | 7% | 0% | 47% | 22% | 0 | $0.5146 |
| openai/gpt-6.1-sol | 87% | 12% | 1% | 73% | 0% | 0 | $0.0703 |
| openai/gpt-6-luna | 85% | 14% | 1% | 83% | 0% | 0 | $0.0108 |
| x-ai/grok-4.7 | 86% | 11% | 3% | 80% | 0% | 0 | $0.4475 |

## Rejected answers

The first few items each model answered invalidly even alone.

### google/gemini-2.5-flash

- Failed to parse response: "frozen green peas": key Some("frozen green peas") is not one of its candidates

### anthropic/claude-sonnet-5.5

- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "The name \"pappardelle or tagliatelle\" offers two interchangeable wide egg pastas, and the only candidate, \"dried tagliatelle\", matches one of them. Pappardelle is similar, so either reading gives the "
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "The name \"coarsely grated monterey jack cheese\" is the same food as the candidate \"monterey jack cheese\". Grating style doesn't change the food. \"shredded monterey jack cheese\" would also fit, but the"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "The name \"top sirloin steak\" matches the first candidate, \"beef top sirloin steak\", which is the plain generic form. The USDA-style keys are narrower: they specify a fat trim, a grade, and lean only v"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "Unsweetened coconut flakes are the same food as shredded unsweetened coconut. Among the candidates, \"dried unsweetened shredded coconut\" is a close match, but \"unsweetened shredded coconut\" and \"shred"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "\"Split vanilla bean\" means a vanilla bean that has been split open, so the food is the whole vanilla bean. The closest candidates are \"vanilla bean\" and \"whole vanilla bean\". \"vanilla bean\" is the pla"

### anthropic/claude-opus-5.5

- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"dried bread crumbs\", \"answer\": \"entry\", \"key\": \"dry bread crumbs\"}]}\n```"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"coarsely grated monterey jack cheese\", \"answer\": \"entry\", \"key\": \"shredded monterey jack cheese\"}]}\n```"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"stock or broth\", \"answer\": \"entry\", \"key\": \"chicken broth or stock\"}]}\n```"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"delicata squash\", \"answer\": \"entry\", \"key\": \"winter squash\"}]}\n```"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"dashi or water\", \"answer\": \"entry\", \"key\": \"dashi\"}]}\n```"

## Worst misses

### google/gemini-2.5-flash

- fun size butterfinger candy bars: Estimate(EstimatedFood { kcal_per_100g: 480.0, grams_per_cup: None, grams_per_piece: Some(21.0) }) (expected candies, nestle, butterfinger bar)
- stock or broth: Entry("beef broth or stock") (expected chicken broth)
- flour tortillas: Entry("flour tortillas, warmed") (expected tortillas, ready-to-bake or -fry, flour, refrigerated)
- fresh or frozen green peas: Entry("frozen green peas") (expected peas, green, raw)
- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 46.0, grams_per_cup: Some(140.0), grams_per_piece: Some(400.0) }) (expected squash, winter, acorn, raw)

### google/gemini-3.8-flash

- stock or broth: Estimate(EstimatedFood { kcal_per_100g: 10.0, grams_per_cup: Some(240.0), grams_per_piece: None }) (expected chicken broth)
- fresh or frozen green peas: Entry("frozen or fresh green peas") (expected peas, green, raw)
- delicata squash: Entry("winter squash") (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- red chili powder or cayenne: Entry("chili powder or cayenne") (expected spices, pepper, red or cayenne)

### google/gemini-3.1-pro-preview

- flour tortillas: Entry("tortillas, ready-to-bake or -fry, flour, shelf stable") (expected tortillas, ready-to-bake or -fry, flour, refrigerated)
- fresh or frozen green peas: Entry("frozen green peas") (expected peas, green, raw)
- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 34.0, grams_per_cup: Some(116.0), grams_per_piece: Some(400.0) }) (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- aluminum-free baking powder: Entry("baking powder") (expected leavening agents, baking powder, double-acting, straight phosphate)

### anthropic/claude-sonnet-5.5

- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- aluminum-free baking powder: Entry("baking powder") (expected leavening agents, baking powder, double-acting, straight phosphate)
- boneless beef chuck: Entry("boneless beef chuck roast") (expected beef, chuck, blade roast, separable lean and fat, trimmed to 1/8" fat, all grades, raw)
- vegetable or chicken stock: Entry("chicken or vegetable stock") (expected soup, vegetable broth, ready to serve)
- water or broth: Entry("broth") (expected water, bottled, generic)

### anthropic/claude-opus-5.5

- fresh or frozen green peas: Entry("frozen or fresh green peas") (expected peas, green, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- red chili powder or cayenne: Entry("chili powder or cayenne") (expected spices, pepper, red or cayenne)
- boneless beef chuck: Entry("boneless beef chuck roast") (expected beef, chuck, blade roast, separable lean and fat, trimmed to 1/8" fat, all grades, raw)
- vegetable or chicken stock: Entry("chicken or vegetable stock") (expected soup, vegetable broth, ready to serve)

### openai/gpt-6.1-sol

- flour tortillas: Entry("tortillas, ready-to-bake or -fry, flour, shelf stable") (expected tortillas, ready-to-bake or -fry, flour, refrigerated)
- fresh or frozen green peas: Entry("frozen or fresh green peas") (expected peas, green, raw)
- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 34.0, grams_per_cup: Some(116.0), grams_per_piece: Some(450.0) }) (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- pickled red onion: Entry("minced white onion or pickled red onions") (expected pickled onions)

### openai/gpt-6-luna

- pappardelle or tagliatelle: Estimate(EstimatedFood { kcal_per_100g: 371.0, grams_per_cup: Some(85.0), grams_per_piece: None }) (expected pasta, dry, unenriched)
- stock or broth: Estimate(EstimatedFood { kcal_per_100g: 6.0, grams_per_cup: Some(240.0), grams_per_piece: None }) (expected chicken broth)
- dry red wine: Entry("dry white or red wine") (expected alcoholic beverage, wine, table, red)
- fresh or frozen green peas: Entry("frozen or fresh green peas") (expected peas, green, raw)
- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 34.0, grams_per_cup: Some(140.0), grams_per_piece: Some(600.0) }) (expected squash, winter, acorn, raw)

### x-ai/grok-4.7

- stock or broth: Estimate(EstimatedFood { kcal_per_100g: 12.0, grams_per_cup: Some(240.0), grams_per_piece: None }) (expected chicken broth)
- flour tortillas: Entry("flour tortillas, warmed") (expected tortillas, ready-to-bake or -fry, flour, refrigerated)
- fresh or frozen green peas: Entry("frozen or fresh green peas") (expected peas, green, raw)
- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 40.0, grams_per_cup: Some(140.0), grams_per_piece: Some(400.0) }) (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
