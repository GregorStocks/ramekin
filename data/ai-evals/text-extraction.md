# AI eval: Text extraction

Written by `make ai-eval`. A recipe read from pasted text (`extract_recipe_from_text`), for 30 pipeline snapshots rendered as the plain text a user would paste (a third wrapped in blog chatter, a third with no description or servings) and 3 texts that aren't recipes. Ingredient lines are compared whole after lowercasing, writing fractions as 1/2, and dropping bullets and extra spaces; section headings are colon-terminated lines and count too. Found is the expected lines answered, extra the answered lines not expected (prose, a neighbouring recipe). Instructions are compared as words: recall is the expected words kept in the instructions or notes, precision the answered instruction words that are expected or allowed (a variation or headnote the source contains). Other text kept is the share of the page's other text (a description, headnote or variation; not a nutrition panel, which photo import has no field for) found in the answer's description, notes or instructions. Invented fields counts difficulty, source, categories (the draft's tags) and rating with words the source never uses, rating unless the source rates it, and servings, times and nutrition with such words or a number the source doesn't state as that kind of quantity (a total summed from the steps, or the 4 of "serves 4" given as minutes), judged by the word each number phrase measures ("5 to 6 minutes", "serves 4"; not "130 degrees"), over all cases. Servings and times kept is the share of the servings and times the source states that the answer gives with the same numbers (a time in any time field). Unsourced note words is the share of the description and notes words that appear nowhere in the source. Columns are over the recipes a model answered validly. "Recipes with warnings" counts valid recipes answered with any warning: text import shows them to the user, so a warning on a sound recipe is noise. "Not a recipe: left empty" counts the non-recipes answered with every field a user would see (title, ingredients, instructions, description, servings, times, nutrition, notes, difficulty, source, categories, rating) empty.

Golden set: `data/ai-evals/golden/text-extraction.json` (33 cases), one call per case, as in production. Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), and are invalid: production doesn't retry them. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Title right | Ingredient lines found | Extra ingredient lines | Instructions recall | Instructions precision | Other text kept | Servings and times kept | Invented fields | Unsourced note words | Invalid | Not a recipe: left empty | Recipes with warnings | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 100% | 100% | 1% | 100% | 100% | 100% | 100% | 0 | 0% | 0% | 100% | 0% | 0 | 0 | $0.0566 |
| google/gemini-3.8-flash | 100% | 100% | 0% | 100% | 100% | 99% | 100% | 0 | 2% | 0% | 100% | 0% | 0 | 0 | $0.1192 |
| openai/gpt-6-luna | 100% | 100% | 2% | 100% | 100% | 99% | 100% | 0 | 0% | 6% | 100% | 0% | 2 | 0 | $0.0126 |
| deepseek/deepseek-v4.1-flash | 100% | 100% | 0% | 100% | 100% | 99% | 100% | 0 | 1% | 0% | 100% | 0% | 0 | 0 | $0.1645 |
| anthropic/claude-sonnet-5.5 | 100% | 100% | 0% | 100% | 100% | 99% | 100% | 0 | 0% | 0% | 100% | 0% | 0 | 0 | $0.4024 |
| openai/gpt-6.1-sol | 100% | 100% | 0% | 100% | 100% | 100% | 100% | 0 | 0% | 6% | 100% | 0% | 2 | 0 | $0.2289 |
| anthropic/claude-opus-5.5 | 100% | 100% | 0% | 100% | 100% | 99% | 100% | 0 | 0% | 0% | 100% | 0% | 0 | 0 | $0.8644 |

## Rejected answers

The first few items each model answered invalidly even alone.

### openai/gpt-6-luna

- Failed to parse response: Unusable model response (finish_reason=Some(content_filter)): "{\""
- Failed to parse response: Unusable model response (finish_reason=Some(content_filter)): "{\"raw_recipe\":{\"title\":\"Red Lentil Soup with Warm Spices\",\"ingredients\":\"4 tablespoons unsalted butter\\n1 large onion , chopped fine\\nSalt and pepper\\n3/4 teaspoon ground coriander\\n1/2 teaspoon gro

### openai/gpt-6.1-sol

- Failed to parse response: Unusable model response (finish_reason=Some(content_filter)): "{\n  \"raw_recipe\": {\n    \"title\": \"Shakshuka With Feta\",\n    \"ingredients\": \"3 tablespoons extra-virgin olive oil\\n1 large onion, halved and thinly sliced\\n1 large red bell pepper, seeded and thinly
- Failed to parse response: Unusable model response (finish_reason=Some(content_filter)): "{\"raw_recipe\":{\"title\":\"Red Lentil Soup with Warm Spices\",\"ingredients\":\"4 tablespoons unsalted butter\\n1 large onion , chopped fine\\nSalt and pepper\\n3/4 teaspoon ground coriander\\n1/2 teaspoon gro

## Worst misses

### google/gemini-2.5-flash

- Grilled Ham and Cheese Sandwiches with Spicy Tomato Soup: missing ["2 tablespoons unsalted butter, plus 6 tablespoons melted"]; extra ["2 tablespoons unsalted butter", "6 tablespoons melted unsalted butter"]; instructions recall 100%, precision 100%
- Braised Ginger Meatballs in Coconut Broth: instructions recall 87%, precision 100%
- Hash Brown Patties: instructions recall 100%, precision 99%
- Chicken, Leek, and Rice Soup: instructions recall 99%, precision 100%
- Focaccia Onion Board: instructions recall 100%, precision 100%

### google/gemini-3.8-flash

- Cold Rice Noodles with Peanut-Lime Chicken: instructions recall 100%, precision 100%; 5 of 28 note/description words not in the source
- Braised Ginger Meatballs in Coconut Broth: instructions recall 100%, precision 100%; 5 of 28 note/description words not in the source
- Takeout-Style Sesame Noodles with Cucumber: instructions recall 100%, precision 100%; 5 of 28 note/description words not in the source
- Son-in-Law Eggs: Thai Fried Hard-Boiled Eggs in Tamarind Sauce Recipe: instructions recall 100%, precision 100%; kept 93% of the other text
- Chicken, Leek, and Rice Soup: instructions recall 99%, precision 100%

### openai/gpt-6-luna

- Hash Brown Patties: extra ["ingredients:"]; instructions recall 100%, precision 100%
- Son-in-Law Eggs: Thai Fried Hard-Boiled Eggs in Tamarind Sauce Recipe: instructions recall 100%, precision 100%; kept 93% of the other text
- Focaccia Onion Board: extra ["ingredients:"]; instructions recall 100%, precision 100%
- Baked Feta with Tomatoes and Chickpeas: extra ["ingredients:"]; instructions recall 100%, precision 100%
- Salted Chocolate Chunk Cookies: extra ["ingredients:"]; instructions recall 100%, precision 100%

### deepseek/deepseek-v4.1-flash

- Takeout-Style Sesame Noodles with Cucumber: instructions recall 100%, precision 100%; 5 of 28 note/description words not in the source
- Son-in-Law Eggs: Thai Fried Hard-Boiled Eggs in Tamarind Sauce Recipe: extra ["ingredients:"]; instructions recall 100%, precision 100%; kept 93% of the other text
- Chicken, Leek, and Rice Soup: instructions recall 99%, precision 100%

### anthropic/claude-sonnet-5.5

- Son-in-Law Eggs: Thai Fried Hard-Boiled Eggs in Tamarind Sauce Recipe: instructions recall 100%, precision 100%; kept 93% of the other text
- Chicken, Leek, and Rice Soup: instructions recall 99%, precision 100%
- Focaccia Onion Board: instructions recall 100%, precision 100%

### openai/gpt-6.1-sol

None.

### anthropic/claude-opus-5.5

- Son-in-Law Eggs: Thai Fried Hard-Boiled Eggs in Tamarind Sauce Recipe: instructions recall 100%, precision 100%; kept 93% of the other text
- Chicken, Leek, and Rice Soup: instructions recall 99%, precision 100%
