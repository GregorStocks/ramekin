# AI eval: Food estimates

Written by `make ai-eval`. Calories (and cup and piece weights) for a food with no catalog candidates (`resolve_ingredient_names`, answer "estimate"), against USDA for 80 foods given by their USDA description. Calorie error is against at least 20 kcal/100 g, so near-zero foods don't dominate. Cup and piece columns count foods where USDA has the weight. The piece is USDA's default portion, which is sometimes a whole pizza, roast or bird where a model gives one slice or serving, so with ~25 cases that column is low-signal.

Golden set: `data/ai-evals/golden/food-estimates.json` (80 cases), asked 40 per call (production asks 40). Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated); a rejected batch is retried as the production worker does (in halves for weights, item by item for names), and a rejected single item is invalid. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | kcal within 20% | kcal median error | kcal P90 error | Cup within 25% | Piece answered | Piece within 25% of answered | Unknown | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 82% | 8% | 34% | 94% | 92% | 48% | 1% | 0% | 0 | 0 | $0.0165 |
| google/gemini-3.8-flash | 90% | 3% | 20% | 94% | 80% | 60% | 0% | 0% | 0 | 0 | $0.0372 |
| google/gemini-3.1-pro-preview | 91% | 4% | 20% | 100% | 84% | 48% | 0% | 0% | 0 | 0 | $0.1479 |
| anthropic/claude-sonnet-5.5 | 86% | 5% | 22% | 97% | 88% | 41% | 0% | 0% | 0 | 0 | $0.0990 |
| anthropic/claude-opus-5.5 | 94% | 0% | 12% | 100% | 84% | 48% | 0% | 0% | 0 | 0 | $0.2240 |
| openai/gpt-6.1-sol | 94% | 0% | 15% | 94% | 96% | 42% | 0% | 0% | 0 | 0 | $0.0624 |
| openai/gpt-6-luna | 81% | 7% | 34% | 97% | 80% | 50% | 0% | 0% | 0 | 0 | $0.0035 |

## Rejected answers

The first few items each model answered invalidly even alone.

## Worst misses

### google/gemini-2.5-flash

- beef, cured, dried: 250 kcal/100 g (USDA 153)
- babyfood, fruit, applesauce and pineapple, junior: 60 kcal/100 g (USDA 39)
- beverages, lemonade-flavor drink, powder, prepared with water: 40 kcal/100 g (USDA 27)
- plums, canned, purple, water pack, solids and liquids: 60 kcal/100 g (USDA 41)
- lamb, new zealand, imported, rack - fully frenched, separable lean and fat, cooked, fast roasted: 280 kcal/100 g (USDA 195)

### google/gemini-3.8-flash

- babyfood, fruit, applesauce and pineapple, junior: 68 kcal/100 g (USDA 39)
- salad dressing, mayonnaise, imitation, soybean: 380 kcal/100 g (USDA 232)
- beverages, lemonade-flavor drink, powder, prepared with water: 41 kcal/100 g (USDA 27)
- lamb, new zealand, imported, rack - fully frenched, separable lean and fat, cooked, fast roasted: 269 kcal/100 g (USDA 195)
- oopah (tunicate), whole animal (alaska native): 45 kcal/100 g (USDA 67)

### google/gemini-3.1-pro-preview

- lamb, new zealand, imported, rack - fully frenched, separable lean and fat, cooked, fast roasted: 280 kcal/100 g (USDA 195)
- beverages, lemonade-flavor drink, powder, prepared with water: 38 kcal/100 g (USDA 27)
- mushrooms, white, microwaved: 25 kcal/100 g (USDA 35)
- babyfood, fruit, applesauce and pineapple, junior: 50 kcal/100 g (USDA 39)
- oopah (tunicate), whole animal (alaska native): 50 kcal/100 g (USDA 67)

### anthropic/claude-sonnet-5.5

- beef, cured, dried: 247 kcal/100 g (USDA 153)
- lamb, new zealand, imported, rack - fully frenched, separable lean and fat, cooked, fast roasted: 290 kcal/100 g (USDA 195)
- beverages, lemonade-flavor drink, powder, prepared with water: 40 kcal/100 g (USDA 27)
- babyfood, fruit, applesauce and pineapple, junior: 55 kcal/100 g (USDA 39)
- beef, chuck, top blade, separable lean and fat, trimmed to 0" fat, select, cooked, broiled: 250 kcal/100 g (USDA 200)

### anthropic/claude-opus-5.5

- lamb, new zealand, imported, rack - fully frenched, separable lean and fat, cooked, fast roasted: 290 kcal/100 g (USDA 195)
- beverages, lemonade-flavor drink, powder, prepared with water: 40 kcal/100 g (USDA 27)
- pork, fresh, loin, blade (roasts), boneless, separable lean only, cooked, roasted: 230 kcal/100 g (USDA 175)
- beef, plate steak, boneless, outside skirt, separable lean and fat, trimmed to 0" fat, all grades, cooked, grilled: 220 kcal/100 g (USDA 292)
- soup, healthy choice garden vegetable soup, canned: 40 kcal/100 g (USDA 51)

### openai/gpt-6.1-sol

- lamb, new zealand, imported, rack - fully frenched, separable lean and fat, cooked, fast roasted: 294 kcal/100 g (USDA 195)
- soup, healthy choice garden vegetable soup, canned: 28 kcal/100 g (USDA 51)
- babyfood, fruit, applesauce and pineapple, junior: 53 kcal/100 g (USDA 39)
- beef, chuck, top blade, separable lean and fat, trimmed to 0" fat, select, cooked, broiled: 250 kcal/100 g (USDA 200)
- pork, fresh, loin, center loin (chops), boneless, separable lean only, cooked, pan-broiled: 195 kcal/100 g (USDA 162)

### openai/gpt-6-luna

- beef, cured, dried: 410 kcal/100 g (USDA 153)
- babyfood, fruit, applesauce and pineapple, junior: 70 kcal/100 g (USDA 39)
- pineapple juice, frozen concentrate, unsweetened, undiluted: 60 kcal/100 g (USDA 179)
- lamb, new zealand, imported, rack - fully frenched, separable lean and fat, cooked, fast roasted: 300 kcal/100 g (USDA 195)
- beverages, lemonade-flavor drink, powder, prepared with water: 40 kcal/100 g (USDA 27)
