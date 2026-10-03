# AI eval: Ingredient weights

Written by `make ai-eval`. Grams in one unit of a catalog food (`estimate_ingredient_weights`), against USDA: 60 densities (cup) and 60 piece weights in estimable units. Error is |estimate − USDA| / USDA; null means the model said there's no typical weight.

Golden set: `data/ai-evals/golden/ingredient-weights.json` (120 cases), asked 1 per call (production asks 40). Truncated calls ran out of the production max_tokens (the batch is then retried item by item here; in production it fails). Cost is what this suite's calls cost at OpenRouter's current prices; a cached rerun spends nothing.

| Model | Within 20% | Within 50% | Median error | P90 error | Null | Invalid | Truncated calls | Cost |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 62% | 84% | 9% | 58% | 2% | 0% | 0 | $0.0305 |
| google/gemini-3.8-flash | 73% | 94% | 4% | 39% | 0% | 0% | 0 | $0.2618 |
| openai/gpt-6.1-sol | 74% | 93% | 2% | 40% | 1% | 0% | 0 | $0.2217 |
| google/gemini-3.1-pro-preview | 72% | 92% | 3% | 46% | 0% | 0% | 0 | $0.6550 |

## Rejected answers

The first few items each model answered invalidly even alone.

## Worst misses

### google/gemini-2.5-flash

- candies, hershey's, almond joy bites | piece: 8 g (USDA 2.2222222222222223 g)
- pork, oriental style, dehydrated | cup: 50 g (USDA 22 g)
- fish, pompano, florida, cooked, dry heat | fillet: 170 g (USDA 88 g)
- danish pastry, fruit, unenriched (includes apple, cinnamon, raisin, strawberry) | piece: 100 g (USDA 53 g)
- soybeans, mature seeds, sprouted, cooked, steamed, with salt | cup: 172 g (USDA 94 g)

### google/gemini-3.8-flash

- pork, oriental style, dehydrated | cup: 85 g (USDA 22 g)
- candies, hershey's, almond joy bites | piece: 6 g (USDA 2.2222222222222223 g)
- duck, young duckling, domesticated, white pekin, leg, meat and skin, bone in, cooked, roasted | leg: 170 g (USDA 92 g)
- restaurant, latino, arroz con grandules (rice and pigeonpeas) | cup: 200 g (USDA 115 g)
- pork, fresh, loin, center rib (chops), boneless, separable lean only, cooked, broiled | chop: 120 g (USDA 71 g)

### openai/gpt-6.1-sol

- pork, oriental style, dehydrated | cup: 80 g (USDA 22 g)
- fish, cusk, cooked, dry heat | fillet: 310 g (USDA 95 g)
- duck, young duckling, domesticated, white pekin, leg, meat and skin, bone in, cooked, roasted | leg: 200 g (USDA 92 g)
- candies, hershey's, almond joy bites | piece: 4.6 g (USDA 2.2222222222222223 g)
- restaurant, latino, arroz con grandules (rice and pigeonpeas) | cup: 200 g (USDA 115 g)

### google/gemini-3.1-pro-preview

- pork, oriental style, dehydrated | cup: 135 g (USDA 22 g)
- candies, hershey's, almond joy bites | piece: 4.5 g (USDA 2.2222222222222223 g)
- fish, cusk, cooked, dry heat | fillet: 159 g (USDA 95 g)
- duck, young duckling, domesticated, white pekin, leg, meat and skin, bone in, cooked, roasted | leg: 150 g (USDA 92 g)
- mustard spinach, (tendergreen), raw | cup: 56 g (USDA 150 g)
