# AI eval: Photo extraction

Written by `make ai-eval`. A recipe read from photos of real pages (`extract_recipe_from_photos`), from `data/ai-evals/photos/` with hand-checked transcriptions; each case's `note` in the golden set says what makes it hard. The photos are sent as uploaded, EXIF rotation and all, as photo import does. With 5 cases, one case moves a column by 20 points. Ingredient lines are compared whole after lowercasing, writing fractions as 1/2, and dropping bullets and extra spaces; section headings are colon-terminated lines and count too. Found is the expected lines answered, extra the answered lines not expected (prose, a neighbouring recipe). Instructions are compared as words: recall is the expected words kept in the instructions (or notes, for a moved tip, once the instructions hold at least half), precision the answered instruction words that are expected or allowed (a variation or headnote the source contains). Other text kept is the share of the page's other text (a description, headnote or variation) found in the answer's description, notes or instructions. Invented fields counts difficulty, source, categories (the draft's tags) and rating with words the source never uses, rating unless the source rates it, and servings, times and nutrition with such words or a number the source doesn't state as that kind of quantity (a total summed from the steps, or the 4 of "serves 4" given as minutes), judged by the word each number phrase measures ("5 to 6 minutes", "serves 4"; not "130 degrees"). A time counts as stated only where the source labels one ("Prep time:", "Total time", "Ready in"), so a step's "chill 30 minutes" given as the total time is invented. Each stated quantity backs one answered value. Counted over all cases. Servings, times and nutrition kept is the share of the servings, times and nutrition the source states that the answer gives with the same numbers (a time in any time field; nutrition in the nutrition field, not the notes). Unsourced note words is the share of the description and notes words that appear nowhere in the source. Columns are over the recipes a model answered validly. Kalbi Burgers has no ingredient list (its ingredients are bold words in the steps), so its expected lines are those words: a judgment call.

Golden set: `data/ai-evals/golden/photo-extraction.json` (5 cases), one call per case, as in production. Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), or a provider error that outlasted its retries, and are invalid: production doesn't retry them. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Title right | Ingredient lines found | Extra ingredient lines | Instructions recall | Instructions precision | Other text kept | Servings, times and nutrition kept | Invented fields | Unsourced note words | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 100% | 96% | 4% | 100% | 100% | 80% | 75% | 4 | 0% | 20% | 1 | 0 | $0.0123 |
| google/gemini-3.8-flash | 100% | 100% | 0% | 100% | 100% | 95% | 90% | 2 | 1% | 0% | 0 | 0 | $0.0427 |
| openai/gpt-6-luna | 80% | 95% | 4% | 100% | 100% | 89% | 100% | 0 | 0% | 0% | 0 | 0 | $0.0113 |
| deepseek/deepseek-v4.1-flash | 100% | 82% | 10% | 99% | 99% | 79% | 80% | 1 | 0% | 0% | 0 | 0 | $0.0246 |
| anthropic/claude-sonnet-5.5 | 100% | 100% | 0% | 100% | 100% | 90% | 100% | 1 | 2% | 0% | 0 | 0 | $0.1081 |
| openai/gpt-6.1-sol | 100% | 100% | 0% | 100% | 100% | 85% | 100% | 1 | 1% | 0% | 0 | 0 | $0.1871 |
| anthropic/claude-opus-5.5 | 100% | 100% | 0% | 100% | 100% | 95% | 100% | 1 | 1% | 0% | 0 | 0 | $0.2406 |

## Rejected answers

The first few items each model answered invalidly even alone.

### google/gemini-2.5-flash

- Failed to parse response: Failed to parse photo_extract response: invalid type: map, expected a string at line 2 column 2; content: "[\n  {\n    \"title\": \"KALBI BURGERS WITH SSAMJANG MAYONNAISE\",\n    \"description\": \"We've brought the robust, savory-sweet flavors of Korean barbecued beef shor

## Worst misses

### google/gemini-2.5-flash

- Grill-Bound Shishkabobs: instructions recall 100%, precision 100%; invented cook_time; kept 25% of the other text
- Potato Doughnuts: missing ["1 1/4 teaspoons table salt, divided", "6 tablespoons hot water, plus extra as needed"]; extra ["1 1/2 teaspoons table salt, divided", "2 tablespoons hot water, plus extra as needed"]; instructions recall 100%, precision 100%; invented nutritional_info; kept 99% of the other text; kept 2 of 3 stated servings, times and nutrition; 1 of 188 note/description words not in the source
- Tacos Chilorio: instructions recall 100%, precision 100%; invented nutritional_info; kept 2 of 3 stated servings, times and nutrition
- Red Rice: instructions recall 100%, precision 100%; invented cook_time; kept 97% of the other text

### google/gemini-3.8-flash

- Tacos Chilorio: instructions recall 100%, precision 100%; invented nutritional_info; kept 2 of 3 stated servings, times and nutrition
- Grill-Bound Shishkabobs: instructions recall 100%, precision 100%; invented cook_time; kept 75% of the other text
- Potato Doughnuts: instructions recall 100%, precision 100%; 4 of 241 note/description words not in the source

### openai/gpt-6-luna

- Grill-Bound Shishkabobs: wrong title; missing ["3 cloves garlic, minced", "2 large onions, sliced"]; extra ["3 onions, sliced"]; instructions recall 98%, precision 98%; kept 50% of the other text
- Red Rice: missing ["2 cups chicken stock (page 133) or water"]; extra ["2 cups chicken stock (page 135) or water"]; instructions recall 100%, precision 100%
- Potato Doughnuts: instructions recall 100%, precision 100%; kept 93% of the other text

### deepseek/deepseek-v4.1-flash

- Kalbi Burgers with Ssamjang Mayonnaise: missing ["1 peeled red onion", "3 tablespoons soy sauce", "4 minced garlic cloves"]; extra ["1. slice four 1/4-inch-thick rounds from 1 peeled red onion and set aside for garnish. grate remaining onion into large bowl and drain off any liquid. stir in 3 tablespoons soy sauce, 4 minced garlic cloves, and 4 teaspoons sugar until sugar is fully dissolved.", "2. add 1 1/2 pounds 85 percent lean ground beef to onion mixture and mix gently until thoroughly combined. gently shape into four 3/4-inch-thick patties, about 4 1/2 inches in diameter. using your thumb, make 1-inch-wide by 1/2-inch-deep depression in center of each patty.", "3. heat 2 teaspoons vegetable oil in 12-inch nonstick skillet over medium-high heat until just smoking. add patties and cook until well browned and meat registers 130 to 135 degrees (for medium), 5 to 6 minutes per side. transfer burgers to plate and tent with aluminum foil."]; instructions recall 99%, precision 99%; kept 97% of the other text; 1 of 78 note/description words not in the source
- Potato Doughnuts: extra ["you'll need 3-inch and 1 1/4-inch round cutters and a dutch oven that holds at least 6 quarts. for the best results, weigh the flour and sugars. heating the oil slowly will make it easier to control the temperature when frying. we like the extra moistness and uniform fluffiness of fresh potatoes, but you can substitute instant mashed potato flakes. skip step 1 and stir 1 1/2 cups (4 ounces/113 grams) flakes with 2 cups boiling water; measure out 2 cups (1 pound/454 grams) and use in place of the potatoes in step 2."]; instructions recall 98%, precision 98%; invented nutritional_info; kept 76% of the other text; kept 1 of 3 stated servings, times and nutrition
- Grill-Bound Shishkabobs: instructions recall 100%, precision 100%; kept 25% of the other text
- Red Rice: instructions recall 100%, precision 100%; kept 97% of the other text

### anthropic/claude-sonnet-5.5

- Grill-Bound Shishkabobs: instructions recall 100%, precision 100%; invented cook_time; kept 50% of the other text; 5 of 5 note/description words not in the source
- Potato Doughnuts: instructions recall 100%, precision 100%; 4 of 241 note/description words not in the source

### openai/gpt-6.1-sol

- Grill-Bound Shishkabobs: instructions recall 100%, precision 100%; invented cook_time; kept 25% of the other text
- Potato Doughnuts: instructions recall 100%, precision 100%; 4 of 241 note/description words not in the source

### anthropic/claude-opus-5.5

- Grill-Bound Shishkabobs: instructions recall 100%, precision 100%; invented cook_time; kept 75% of the other text
- Potato Doughnuts: instructions recall 100%, precision 100%; 4 of 241 note/description words not in the source
