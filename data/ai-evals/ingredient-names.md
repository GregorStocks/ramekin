# AI eval: Ingredient names

Written by `make ai-eval`. Picking the catalog food a name means from its candidates (`resolve_ingredient_names`), for 100 curated aliases (correct if the answer resolves to the alias's food, or an equivalent entry: the same USDA food or calories within 10%, since the catalog has several entries for many foods; the alias itself is not offered) and 30 curated not-food names. Some curated aliases are judgment calls (delicata squash counts as acorn squash), so a sensible answer can score as wrong: compare models with each other rather than reading this as absolute accuracy.

Golden set: `data/ai-evals/golden/ingredient-names.json` (130 cases), asked 40 per call (production asks 40). Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated); a rejected batch is retried as the production worker does (in halves for weights, item by item for names), and a rejected single item is invalid. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Correct food | Wrong food | Unknown | Not food right | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 89% | 8% | 1% | 97% | 2% | 4 | 0 | $0.0340 |
| google/gemini-3.8-flash | 90% | 10% | 0% | 97% | 0% | 1 | 0 | $0.1679 |
| google/gemini-3.1-pro-preview | 93% | 7% | 0% | 97% | 0% | 0 | 0 | $0.3162 |
| anthropic/claude-sonnet-5.5 | 93% | 7% | 0% | 97% | 0% | 1 | 0 | $0.2035 |
| anthropic/claude-opus-5.5 | 94% | 6% | 0% | 97% | 0% | 0 | 0 | $0.2842 |
| openai/gpt-6.1-sol | 93% | 6% | 1% | 97% | 0% | 0 | 0 | $0.0745 |
| openai/gpt-6-luna | 91% | 9% | 0% | 93% | 0% | 1 | 0 | $0.0081 |

## Rejected answers

The first few items each model answered invalidly even alone.

### google/gemini-2.5-flash

- Failed to parse response: "fresh or frozen green peas": key Some("fresh or frozen green peas") is not one of its candidates
- Failed to parse response: "frozen green peas": key Some("frozen green peas") is not one of its candidates

## Worst misses

### google/gemini-2.5-flash

- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 46.0, grams_per_cup: Some(140.0), grams_per_piece: Some(300.0) }) (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- red chili powder or cayenne: Entry("chili powder or cayenne") (expected spices, pepper, red or cayenne)
- container frozen whipped topping: Entry("whipped topping, frozen, low fat") (expected dessert topping, semi solid, frozen)
- boneless beef chuck: Entry("boneless beef chuck roast") (expected beef, chuck, blade roast, separable lean and fat, trimmed to 1/8" fat, all grades, raw)

### google/gemini-3.8-flash

- stock or broth: Estimate(EstimatedFood { kcal_per_100g: 10.0, grams_per_cup: Some(240.0), grams_per_piece: None }) (expected chicken broth)
- delicata squash: Entry("winter squash") (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- pickled red onion: Estimate(EstimatedFood { kcal_per_100g: 50.0, grams_per_cup: Some(160.0), grams_per_piece: None }) (expected pickled onions)
- boneless beef chuck: Entry("boneless beef chuck roast") (expected beef, chuck, blade roast, separable lean and fat, trimmed to 1/8" fat, all grades, raw)

### google/gemini-3.1-pro-preview

- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 34.0, grams_per_cup: Some(116.0), grams_per_piece: Some(400.0) }) (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- boneless beef chuck: Entry("boneless beef chuck roast") (expected beef, chuck, blade roast, separable lean and fat, trimmed to 1/8" fat, all grades, raw)
- vegetable or chicken stock: Entry("chicken or vegetable stock") (expected soup, vegetable broth, ready to serve)
- frozen lima beans: Entry("lima beans, immature seeds, frozen, baby, unprepared") (expected lima beans, immature seeds, frozen, fordhook, unprepared)

### anthropic/claude-sonnet-5.5

- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 40.0, grams_per_cup: Some(130.0), grams_per_piece: Some(300.0) }) (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- red chili powder or cayenne: Entry("chili powder or cayenne") (expected spices, pepper, red or cayenne)
- boneless beef chuck: Entry("boneless beef chuck roast") (expected beef, chuck, blade roast, separable lean and fat, trimmed to 1/8" fat, all grades, raw)
- vegetable or chicken stock: Entry("chicken or vegetable stock") (expected soup, vegetable broth, ready to serve)

### anthropic/claude-opus-5.5

- delicata squash: Entry("winter squash") (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- boneless beef chuck: Entry("boneless beef chuck roast") (expected beef, chuck, blade roast, separable lean and fat, trimmed to 1/8" fat, all grades, raw)
- vegetable or chicken stock: Entry("chicken or vegetable stock") (expected soup, vegetable broth, ready to serve)
- frozen lima beans: Entry("lima beans, immature seeds, frozen, baby, unprepared") (expected lima beans, immature seeds, frozen, fordhook, unprepared)

### openai/gpt-6.1-sol

- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 34.0, grams_per_cup: Some(140.0), grams_per_piece: Some(500.0) }) (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- boneless beef chuck: Entry("boneless beef chuck roast") (expected beef, chuck, blade roast, separable lean and fat, trimmed to 1/8" fat, all grades, raw)
- vegetable or chicken stock: Entry("chicken or vegetable stock") (expected soup, vegetable broth, ready to serve)
- whole tomatoes: Entry("tomatoes") (expected tomatoes, red, ripe, canned, packed in tomato juice)

### openai/gpt-6-luna

- stock or broth: Estimate(EstimatedFood { kcal_per_100g: 5.0, grams_per_cup: Some(240.0), grams_per_piece: None }) (expected chicken broth)
- delicata squash: Estimate(EstimatedFood { kcal_per_100g: 34.0, grams_per_cup: Some(140.0), grams_per_piece: Some(600.0) }) (expected squash, winter, acorn, raw)
- angostura bitters: Entry("angostura aromatic bitters") (expected bitters)
- red chili powder or cayenne: Entry("chili powder or cayenne") (expected spices, pepper, red or cayenne)
- pickled red onion: Entry("minced white onion or pickled red onions") (expected pickled onions)
