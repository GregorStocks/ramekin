# AI eval: Food estimates

Written by `make ai-eval`. Calories (and cup and piece weights) for a food with no catalog candidates (`resolve_ingredient_names`, answer "estimate"), against USDA for 80 foods given by their USDA description. Calorie error is against at least 20 kcal/100 g, so near-zero foods don't dominate. Cup and piece columns count foods where USDA has the weight; the piece is USDA's default portion, which isn't always a whole item.

Golden set: `data/ai-evals/golden/food-estimates.json` (80 cases), asked 40 per call (production asks 40). Truncated calls ran out of the production max_tokens (the batch is then retried item by item here; in production it fails). Cost is what this suite's calls cost at OpenRouter's current prices; a cached rerun spends nothing.

| Model | kcal within 20% | kcal median error | kcal P90 error | Cup within 25% | Piece within 25% | Unknown | Invalid | Truncated calls | Cost |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 81% | 8% | 37% | 94% | 28% | 1% | 0% | 0 | $0.0162 |
| google/gemini-3.8-flash | 91% | 2% | 20% | 94% | 40% | 0% | 0% | 0 | $0.0272 |
| google/gemini-3.1-pro-preview | 92% | 3% | 16% | 100% | 32% | 0% | 0% | 0 | $0.1386 |
| anthropic/claude-sonnet-5.5 | 52% | 5% | 20% | 58% | 20% | 0% | 41% | 0 | $0.0500 |
| anthropic/claude-opus-5.5 | 82% | 0% | 15% | 81% | 20% | 0% | 14% | 0 | $0.3320 |
| openai/gpt-6.1-sol | 89% | 0% | 20% | 94% | 20% | 0% | 0% | 0 | $0.0607 |
| openai/gpt-6-luna | 84% | 6% | 29% | 100% | 20% | 0% | 0% | 0 | $0.0035 |
| x-ai/grok-4.7 | 91% | 2% | 17% | 97% | 32% | 0% | 0% | 0 | $0.1184 |

## Rejected answers

The first few items each model answered invalidly even alone.

### anthropic/claude-sonnet-5.5

- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "The item is not marked ambiguous and has no candidates, so \"entry\" isn't possible. It names a real food, canned purple plums in water pack with solids and liquids, so I'm answering \"estimate\".\n\nCanned"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "Candidates are empty, so \"entry\" isn't possible. The item is a real food: cooked catjang cowpeas (USDA name), boiled with salt. Typical values are about 116 kcal/100g, similar to cooked cowpeas/blacke"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "Candidates are empty, so \"entry\" is not possible. The item is a real food (wild rabbit, cooked and stewed), not marked ambiguous, so \"estimate\" is the right answer.\n\nFor the values, I used the USDA fi"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "The candidate list is empty, so \"entry\" is not possible. The name is a real food, cooked skipjack tuna (USDA style), so \"estimate\" fits. Cooked skipjack tuna has about 132 kcal per 100 g. Flaked fish "
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "Candidates are empty, so \"entry\" isn't possible. The item is a real food (roasted antelope meat), not marked ambiguous, so I'll give an estimate.\n\nPer the USDA entry for game meat, antelope, cooked, r"

### anthropic/claude-opus-5.5

- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"tempeh\", \"answer\": \"estimate\", \"kcal_per_100g\": 192, \"grams_per_cup\": 166, \"grams_per_piece\": 227}]}\n```"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"sausage, vienna, canned, chicken, beef, pork\", \"answer\": \"estimate\", \"kcal_per_100g\": 230, \"grams_per_cup\": null, \"grams_per_piece\": 16}]}\n```"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"babyfood, fruit, apricot with tapioca, strained\", \"answer\": \"estimate\", \"kcal_per_100g\": 63, \"grams_per_cup\": 245, \"grams_per_piece\": null}]}\n```"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"rice, white, long-grain, regular, enriched, cooked\", \"answer\": \"estimate\", \"kcal_per_100g\": 130, \"grams_per_cup\": 158, \"grams_per_piece\": null}]}\n```"
- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\"resolutions\": [{\"name\": \"ham and cheese spread\", \"answer\": \"estimate\", \"kcal_per_100g\": 280, \"grams_per_cup\": 230, \"grams_per_piece\": null}]}\n```"

## Worst misses

### google/gemini-2.5-flash

- beef, cured, dried: 250 kcal/100 g (USDA 153)
- babyfood, fruit, applesauce and pineapple, junior: 60 kcal/100 g (USDA 39)
- salad dressing, mayonnaise, imitation, soybean: 350 kcal/100 g (USDA 232)
- beverages, lemonade-flavor drink, powder, prepared with water: 40 kcal/100 g (USDA 27)
- plums, canned, purple, water pack, solids and liquids: 60 kcal/100 g (USDA 41)

### google/gemini-3.8-flash

- babyfood, fruit, applesauce and pineapple, junior: 75 kcal/100 g (USDA 39)
- salad dressing, mayonnaise, imitation, soybean: 348 kcal/100 g (USDA 232)
- lamb, new zealand, imported, rack - fully frenched, separable lean and fat, cooked, fast roasted: 277 kcal/100 g (USDA 195)
- beef, cured, dried: 204 kcal/100 g (USDA 153)
- beverages, lemonade-flavor drink, powder, prepared with water: 35 kcal/100 g (USDA 27)

### google/gemini-3.1-pro-preview

- lamb, new zealand, imported, rack - fully frenched, separable lean and fat, cooked, fast roasted: 300 kcal/100 g (USDA 195)
- beverages, lemonade-flavor drink, powder, prepared with water: 38 kcal/100 g (USDA 27)
- oopah (tunicate), whole animal (alaska native): 50 kcal/100 g (USDA 67)
- tomatoes, red, ripe, canned, packed in tomato juice: 21 kcal/100 g (USDA 16)
- pork, fresh, loin, center loin (chops), boneless, separable lean only, cooked, pan-broiled: 200 kcal/100 g (USDA 162)

### anthropic/claude-sonnet-5.5

- gravy, turkey, canned, ready-to-serve: 25 kcal/100 g (USDA 51)
- lamb, new zealand, imported, rack - fully frenched, separable lean and fat, cooked, fast roasted: 280 kcal/100 g (USDA 195)
- mustard greens, cooked, boiled, drained, with salt: 15 kcal/100 g (USDA 26)
- babyfood, fruit, applesauce and pineapple, junior: 50 kcal/100 g (USDA 39)
- ham, chopped, canned: 190 kcal/100 g (USDA 239)

### anthropic/claude-opus-5.5

- lamb, new zealand, imported, rack - fully frenched, separable lean and fat, cooked, fast roasted: 300 kcal/100 g (USDA 195)
- beverages, lemonade-flavor drink, powder, prepared with water: 34 kcal/100 g (USDA 27)
- soup, healthy choice garden vegetable soup, canned: 40 kcal/100 g (USDA 51)
- pork, cured, ham with natural juices, spiral slice, meat only, boneless, separable lean only, heated, roasted: 151 kcal/100 g (USDA 126)
- oopah (tunicate), whole animal (alaska native): 55 kcal/100 g (USDA 67)

### openai/gpt-6.1-sol

- lamb, new zealand, imported, rack - fully frenched, separable lean and fat, cooked, fast roasted: 320 kcal/100 g (USDA 195)
- bread, blue corn, somiviki (hopi): 266 kcal/100 g (USDA 186)
- soup, healthy choice garden vegetable soup, canned: 30 kcal/100 g (USDA 51)
- salad dressing, mayonnaise, imitation, soybean: 321 kcal/100 g (USDA 232)
- beef, chuck, top blade, separable lean and fat, trimmed to 0" fat, select, cooked, broiled: 253 kcal/100 g (USDA 200)

### openai/gpt-6-luna

- beef, cured, dried: 410 kcal/100 g (USDA 153)
- babyfood, fruit, applesauce and pineapple, junior: 65 kcal/100 g (USDA 39)
- beverages, lemonade-flavor drink, powder, prepared with water: 40 kcal/100 g (USDA 27)
- lamb, new zealand, imported, rack - fully frenched, separable lean and fat, cooked, fast roasted: 287 kcal/100 g (USDA 195)
- salad dressing, mayonnaise, imitation, soybean: 333 kcal/100 g (USDA 232)

### x-ai/grok-4.7

- babyfood, fruit, applesauce and pineapple, junior: 63 kcal/100 g (USDA 39)
- lamb, new zealand, imported, rack - fully frenched, separable lean and fat, cooked, fast roasted: 292 kcal/100 g (USDA 195)
- restaurant, chinese, vegetable lo mein, without meat: 157 kcal/100 g (USDA 121)
- veal, composite of trimmed retail cuts, separable lean only, raw: 144 kcal/100 g (USDA 112)
- beverages, lemonade-flavor drink, powder, prepared with water: 34 kcal/100 g (USDA 27)
