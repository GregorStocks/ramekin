# AI eval: Food estimates

Written by `make ai-eval`. Calories (and cup and piece weights) for a food with no catalog candidates (`resolve_ingredient_names`, answer "estimate"), against USDA for 80 foods given by their USDA description. Calorie error is against at least 20 kcal/100 g, so near-zero foods don't dominate. Cup and piece columns count foods where USDA has the weight; the piece is USDA's default portion, which isn't always a whole item.

Golden set: `data/ai-evals/golden/food-estimates.json` (80 cases), asked 1 per call (production asks 40). Truncated calls ran out of the production max_tokens (the batch is then retried item by item here; in production it fails). Cost is what this suite's calls cost at OpenRouter's current prices; a cached rerun spends nothing.

| Model | kcal within 20% | kcal median error | kcal P90 error | Cup within 25% | Piece within 25% | Unknown | Invalid | Truncated calls | Cost |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 41% | 7% | 46% | 55% | 12% | 46% | 0% | 0 | $0.0271 |
| google/gemini-3.8-flash | 88% | 3% | 23% | 97% | 40% | 0% | 0% | 0 | $0.1786 |
| openai/gpt-6.1-sol | 88% | 0% | 21% | 97% | 40% | 0% | 0% | 0 | $0.1999 |
| google/gemini-3.1-pro-preview | 91% | 4% | 20% | 100% | 36% | 0% | 0% | 0 | $0.7273 |

## Rejected answers

The first few items each model answered invalidly even alone.

## Worst misses

### google/gemini-2.5-flash

- beef, cured, dried: 250 kcal/100 g (USDA 153)
- salad dressing, mayonnaise, imitation, soybean: 370 kcal/100 g (USDA 232)
- babyfood, fruit, applesauce and pineapple, junior: 60 kcal/100 g (USDA 39)
- beverages, lemonade-flavor drink, powder, prepared with water: 40 kcal/100 g (USDA 27)
- plums, canned, purple, water pack, solids and liquids: 60 kcal/100 g (USDA 41)

### google/gemini-3.8-flash

- babyfood, fruit, applesauce and pineapple, junior: 75 kcal/100 g (USDA 39)
- oopah (tunicate), whole animal (alaska native): 35 kcal/100 g (USDA 67)
- lamb, new zealand, imported, rack - fully frenched, separable lean and fat, cooked, fast roasted: 279 kcal/100 g (USDA 195)
- beverages, lemonade-flavor drink, powder, prepared with water: 37 kcal/100 g (USDA 27)
- beef, cured, dried: 205 kcal/100 g (USDA 153)

### openai/gpt-6.1-sol

- lamb, new zealand, imported, rack - fully frenched, separable lean and fat, cooked, fast roasted: 310 kcal/100 g (USDA 195)
- bread, blue corn, somiviki (hopi): 276 kcal/100 g (USDA 186)
- soup, healthy choice garden vegetable soup, canned: 30 kcal/100 g (USDA 51)
- babyfood, fruit, applesauce and pineapple, junior: 50 kcal/100 g (USDA 39)
- pork, fresh, loin, blade (roasts), boneless, separable lean only, cooked, roasted: 220 kcal/100 g (USDA 175)

### google/gemini-3.1-pro-preview

- beef, cured, dried: 410 kcal/100 g (USDA 153)
- lamb, new zealand, imported, rack - fully frenched, separable lean and fat, cooked, fast roasted: 338 kcal/100 g (USDA 195)
- beverages, lemonade-flavor drink, powder, prepared with water: 38 kcal/100 g (USDA 27)
- soup, healthy choice garden vegetable soup, canned: 37 kcal/100 g (USDA 51)
- pork, fresh, loin, center loin (chops), boneless, separable lean only, cooked, pan-broiled: 200 kcal/100 g (USDA 162)
