# AI eval: Ingredient names

Written by `make ai-eval`. Picking the catalog food a name means from its candidates (`resolve_ingredient_names`), for 100 curated aliases (correct if the answer resolves to the alias's food; the alias itself is not offered) and 30 curated not-food names. Some curated aliases are judgment calls (delicata squash counts as acorn squash), so a sensible answer can score as wrong: compare models with each other rather than reading this as absolute accuracy.

Golden set: `data/ai-evals/golden/ingredient-names.json` (130 cases), asked 40 per call (production asks 40). Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated; production treats those as provider errors); a rejected batch is retried item by item. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Correct food | Wrong food | Unknown | Not food right | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 81% | 17% | 1% | 57% | 1% | 2 | 0 | $0.0242 |
| google/gemini-3.8-flash | 86% | 14% | 0% | 83% | 0% | 1 | 1 | $0.1856 |
| google/gemini-3.1-pro-preview | 87% | 13% | 0% | 87% | 0% | 0 | 0 | $0.3260 |
| anthropic/claude-sonnet-5.5 | 70% | 7% | 1% | 80% | 17% | 23 | 0 | $0.1129 |
| anthropic/claude-opus-5.5 | 86% | 8% | 0% | 47% | 12% | 19 | 0 | $0.4590 |
| openai/gpt-6.1-sol | 87% | 12% | 1% | 73% | 0% | 0 | 0 | $0.0703 |
| openai/gpt-6-luna | 85% | 14% | 1% | 83% | 0% | 1 | 0 | $0.0079 |
| x-ai/grok-4.7 | failed: API error: Request timed out after 120s | – | – | – | – | 0 | 0 | $0.0000 |

## Rejected answers

The first few items each model answered invalidly even alone.

### google/gemini-2.5-flash

- Failed to parse response: "frozen green peas": key Some("frozen green peas") is not one of its candidates

### anthropic/claude-sonnet-5.5

- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "The name \"coarsely grated monterey jack cheese\" means the same food as the candidate \"monterey jack cheese\". Candidates such as \"shredded monterey jack cheese\" would also fit, but the plain \"monterey "
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "The name \"top sirloin steak\" matches the first candidate, \"beef top sirloin steak\", which is the plain generic entry. A recipe using this name most likely means raw steak, and the generic key doesn't "
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "Unsweetened coconut flakes are the same food as shredded unsweetened coconut. Several candidates fit that description, so I picked the plainest one, \"shredded unsweetened coconut\", which doesn't add a"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "The name offers two alternatives: provolone cheese or fresh mozzarella. Both are candidates, so either would be a valid match. I'm picking \"provolone cheese\" because it is listed first in the name and"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "The name \"whole milk ricotta\" means the same food as several candidates. I chose \"whole milk ricotta cheese\" because it is the closest wording to the recipe name.\n\n{\"resolutions\": [{\"name\": \"whole mil"

### anthropic/claude-opus-5.5

- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"pie crust\", \"answer\": \"entry\", \"key\": \"pie crust, refrigerated, regular, unbaked\"}]}\n```"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"aluminum-free baking powder\", \"answer\": \"entry\", \"key\": \"baking powder\"}]}\n```"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"smoked paprika\", \"answer\": \"entry\", \"key\": \"smoked paprika powder\"}]}\n```"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"whole tomatoes\", \"answer\": \"entry\", \"key\": \"canned whole tomatoes\"}]}\n```"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"water or broth\", \"answer\": \"entry\", \"key\": \"water\"}]}\n```"

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

- flour tortillas: Entry("tortillas, ready-to-bake or -fry, flour, shelf stable") (expected tortillas, ready-to-bake or -fry, flour, refrigerated)
- fresh or frozen green peas: Entry("frozen or fresh green peas") (expected peas, green, raw)
- delicata squash: Entry("winter squash") (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- boneless beef chuck: Entry("boneless beef chuck roast") (expected beef, chuck, blade roast, separable lean and fat, trimmed to 1/8" fat, all grades, raw)

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

None.
