# AI eval: Ingredient names

Written by `make ai-eval`. Picking the catalog food a name means from its candidates (`resolve_ingredient_names`), for 100 curated aliases (correct if the answer resolves to the alias's food; the alias itself is not offered) and 30 curated not-food names. Some curated aliases are judgment calls (delicata squash counts as acorn squash), so a sensible answer can score as wrong: compare models with each other rather than reading this as absolute accuracy.

Golden set: `data/ai-evals/golden/ingredient-names.json` (130 cases), asked 40 per call (production asks 40). Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated); a rejected batch is retried as the production worker does (in halves for weights, item by item for names), and a rejected single item is invalid. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Correct food | Wrong food | Unknown | Not food right | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 81% | 17% | 1% | 57% | 1% | 2 | 0 | $0.0242 |
| google/gemini-3.8-flash | 86% | 14% | 0% | 83% | 0% | 1 | 0 | $0.1856 |
| google/gemini-3.1-pro-preview | 87% | 13% | 0% | 87% | 0% | 0 | 0 | $0.3260 |
| anthropic/claude-sonnet-5.5 | 70% | 7% | 1% | 80% | 17% | 23 | 0 | $0.1129 |
| anthropic/claude-opus-5.5 | 89% | 9% | 0% | 53% | 8% | 12 | 0 | $0.3847 |
| openai/gpt-6.1-sol | 87% | 12% | 1% | 73% | 0% | 0 | 0 | $0.0703 |
| openai/gpt-6-luna | 85% | 14% | 1% | 83% | 0% | 1 | 0 | $0.0079 |
| x-ai/grok-4.7 | 86% | 11% | 3% | 80% | 0% | 1 | 0 | $0.4475 |

## Rejected answers

The first few items each model answered invalidly even alone.

### google/gemini-2.5-flash

- Failed to parse response: "frozen green peas": key Some("frozen green peas") is not one of its candidates

### anthropic/claude-sonnet-5.5

- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "The name \"coarsely grated monterey jack cheese\" is the same food as the candidate \"monterey jack cheese\". The other monterey jack candidates carry extra words (\"or pepper jack\", \"or cheddar\"). \"shredd"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "The name \"top sirloin steak\" matches the first candidate, \"beef top sirloin steak\", which is the plain generic form. The other candidates are specific USDA cuts with grade and trim details, so I chose"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "\"Unsweetened coconut flakes\" most likely means dried unsweetened shredded coconut. Of the candidates, \"dried unsweetened shredded coconut\" is the most specific and fits best, since flakes are dried. \""
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "The name offers two alternatives: provolone cheese or fresh mozzarella. Both appear among the candidates, so either would be a valid match. I picked \"provolone cheese\" because it is the first one name"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "The name \"whole milk ricotta\" means the same food as several candidates. I picked \"whole milk ricotta cheese\" because its wording is closest to the item name.\n\n{\"resolutions\": [{\"name\": \"whole milk ri"

### anthropic/claude-opus-5.5

- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"water or broth\", \"answer\": \"entry\", \"key\": \"water or low-sodium broth\"}]}\n```"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"escarole\", \"answer\": \"entry\", \"key\": \"roughly chopped escarole, tuscan kale or radicchio\"}]}\n```"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "\"Dough and baking\" reads like a section heading in an ingredient list, not an actual ingredient.\n\n```json\n{\"resolutions\": [{\"name\": \"dough and baking\", \"answer\": \"not_food\"}]}\n```"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"crushed, diced\", \"answer\": \"unknown\"}]}\n```"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"toppings of your choice\", \"answer\": \"unknown\"}]}\n```"

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
- aluminum-free baking powder: Entry("baking powder") (expected leavening agents, baking powder, double-acting, straight phosphate)

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
