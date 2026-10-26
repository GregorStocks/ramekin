//! House style: rewrite a recipe's ingredients and steps to the owner's style
//! guide (`doc/house-style.md`) with the Edit with AI tool loop, restricted to
//! the tools that touch ingredients and steps.
//!
//! `finish` is accepted only once every amount and every catalog food the
//! recipe started with is still there, so a rewrite can reword, reorder and
//! move prep into steps but can't change what the recipe makes. Amounts
//! compare as the parser reads them, and a line keeps its food when it only
//! drops words from it without changing the kind of food (see
//! `Line::kept_by`).

use std::collections::BTreeSet;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use crate::ai::edit_recipe::{run_edit_loop, EditPrompt};
use crate::ai::edit_recipe_tools::{edit_recipe_tools, format_amount, RecipeDraft, FINISH_TOOL};
use crate::ai::prompts::house_style::{
    render_house_style_system_prompt, render_house_style_user_prompt, HOUSE_STYLE_PROMPT_NAME,
};
use crate::ai::{AiClient, AiError, EditRecipeResult, NoUserData};
use crate::catalog::{self, Entry, Resolution};
use crate::ingredient_parser::quantity;

/// The tools a house-style rewrite may call.
const HOUSE_STYLE_TOOLS: &[&str] = &[
    "set_ingredient",
    "add_ingredient",
    "remove_ingredient",
    "move_ingredient",
    "set_section",
    "set_step",
    "add_step",
    "remove_step",
    FINISH_TOOL,
];

/// Rewrite `recipe` into the house style. `recipe` serializes to a JSON
/// object with the server's recipe fields; the rewrite must parse back into
/// `T`.
pub async fn house_style<T: Serialize + DeserializeOwned>(
    ai_client: &dyn AiClient,
    recipe: &T,
) -> Result<EditRecipeResult<T>, AiError> {
    let value = serde_json::to_value(recipe)
        .map_err(|e| AiError::ParseError(format!("Recipe doesn't serialize: {e}")))?;
    let original = value["ingredients"]
        .as_array()
        .map_or(&[][..], Vec::as_slice);
    let before = Before {
        measures: measures(original),
        lines: original.iter().map(Line::of).collect(),
    };

    let tools = edit_recipe_tools()
        .into_iter()
        .filter(|tool| HOUSE_STYLE_TOOLS.contains(&tool.name.as_str()))
        .collect();
    let check = move |draft: &RecipeDraft| check_unchanged(&before, draft.ingredients());
    run_edit_loop(
        ai_client,
        recipe,
        EditPrompt {
            name: HOUSE_STYLE_PROMPT_NAME,
            system: render_house_style_system_prompt(),
            tools,
            check: Some(Box::new(check)),
        },
        render_house_style_user_prompt,
        Vec::new(),
        &NoUserData,
    )
    .await
}

/// Every ingredient's amounts, as sorted (amount, unit) pairs, so reordering
/// doesn't count as a change.
fn measures(ingredients: &[Value]) -> Vec<(String, String)> {
    let mut measures: Vec<(String, String)> = ingredients.iter().flat_map(line_measures).collect();
    measures.sort();
    measures
}

/// One ingredient's amounts as (amount, unit) pairs, sorted. An amount the
/// parser reads compares as a number or range ("1/2" and "0.5", "2-3" and "2
/// to 3" are the same), and a container of a stated size as its count and
/// one's size ("2 14.5-ounce cans" and "2 (14.5 oz) cans" are the same).
fn line_measures(ingredient: &Value) -> Vec<(String, String)> {
    let mut measures: Vec<(String, String)> = ingredient
        .get("measurements")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|m| {
            let text = |key: &str| {
                m.get(key)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_lowercase()
            };
            let (amount, unit) = (text("amount"), text("unit"));
            if amount.is_empty() && unit.is_empty() {
                return Vec::new();
            }
            match quantity::split_package(&unit) {
                Some((container, size, size_unit)) => vec![
                    (number(&amount), container),
                    (number(&format_range(size)), size_unit),
                ],
                None => vec![(number(&amount), unit)],
            }
        })
        .collect();
    measures.sort();
    measures
}

/// An amount as a number or range, when the parser reads it.
fn number(amount: &str) -> String {
    quantity::range(amount).map_or_else(|| amount.to_string(), format_range)
}

fn format_range(range: quantity::Range) -> String {
    if range.min == range.max {
        format_amount(range.min)
    } else {
        format!("{}-{}", format_amount(range.min), format_amount(range.max))
    }
}

/// An ingredient line, as what it must keep through a rewrite.
struct Line {
    /// The catalog foods it names, or none when the catalog can't tell.
    foods: Vec<&'static Entry>,
    /// Its item's and note's words.
    words: BTreeSet<String>,
    /// Its amounts, as text.
    amounts: String,
}

impl Line {
    fn of(ingredient: &Value) -> Line {
        let item = ingredient.get("item").and_then(Value::as_str).unwrap_or("");
        let note = ingredient.get("note").and_then(Value::as_str);
        let foods = match catalog::resolve_line(item, note) {
            Resolution::Entry { entry, .. } => vec![entry],
            Resolution::Compound(entries) => entries,
            _ => Vec::new(),
        };
        let words = [item, note.unwrap_or("")]
            .iter()
            .flat_map(|text| text.split(|c: char| !c.is_alphanumeric()))
            .filter(|word| !word.is_empty())
            .map(str::to_lowercase)
            .collect();
        let amounts = line_measures(ingredient)
            .into_iter()
            .map(|(amount, unit)| format!("{amount} {unit}").trim().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        Line {
            foods,
            words,
            amounts,
        }
    }

    fn food_names(&self) -> String {
        self.foods
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>()
            .join(" + ")
    }

    /// Whether `rewritten` still names this line's food: the same catalog
    /// entries, or a line that only drops prep from this one and names the
    /// same family of food or one the catalog doesn't know ("6 hardboiled
    /// eggs" to "6 eggs", "frisée, torn into bite-size pieces" to "frisée").
    /// Only prep and restated amounts may go (`is_droppable_word`), so a
    /// dropped word can't change the food ("garlic powder" to "garlic" or
    /// "powder", "sweetened condensed milk" to "milk").
    fn kept_by(&self, rewritten: &Line) -> bool {
        let ids = |line: &Line| {
            line.foods
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>()
        };
        let families = |line: &Line| {
            let mut families: Vec<&str> = line.foods.iter().map(|entry| family(entry)).collect();
            families.sort();
            families
        };
        ids(self) == ids(rewritten)
            || (!rewritten.words.is_empty()
                && rewritten.words.is_subset(&self.words)
                && self
                    .words
                    .difference(&rewritten.words)
                    .all(|word| is_droppable_word(word))
                && (rewritten.foods.is_empty() || families(self) == families(rewritten)))
    }
}

/// Words a rewrite may drop from a line: prep that moves into the steps (the
/// style guide's cutting, chopping, peeling, boiling and so on) with the
/// words that say how, and an amount restated in the note ("about 2
/// ounces"), since the amounts are checked on their own. Purchased forms
/// ("ground", "dried", "frozen", "shredded", "crumbled") and states that
/// change the measure ("cooked") aren't here: they stay in the line.
fn is_droppable_word(word: &str) -> bool {
    const WORDS: &[&str] = &[
        "about", "approximately", "g", "grams", "lb", "lbs", "ounce", "ounces", "oz", "pound",
        "pounds",
        "and", "beaten", "bite", "boiled", "chopped", "chunks", "coarsely", "cored", "crosswise",
        "cubed", "cubes", "cut", "diced", "finely", "grated", "halved", "hard", "hardboiled",
        "inch", "into", "julienned", "lengthwise", "lightly", "melted", "minced", "peeled",
        "piece", "pieces", "quartered", "roughly", "rounds", "seeded", "size", "sized",
        "sliced", "slices", "softened", "strips", "thick", "thin", "thinly", "toasted", "torn",
        "trimmed", "wedges", "whisked",
    ];
    WORDS.contains(&word) || word.chars().all(|c| c.is_ascii_digit())
}

/// The kind of food a catalog entry is: a USDA description's first part
/// ("egg" for "egg, whole, cooked, hard-boiled"), or a curated entry's name.
fn family(entry: &Entry) -> &str {
    entry.id.split(',').next().unwrap_or(&entry.id).trim()
}

/// What a rewrite must keep, from the original ingredients.
struct Before {
    measures: Vec<(String, String)>,
    lines: Vec<Line>,
}

/// Whether the rewritten `ingredients` keep the original amounts and foods;
/// the error says what's missing, for the model to fix.
fn check_unchanged(before: &Before, ingredients: &[Value]) -> Result<(), String> {
    let measures_after = measures(ingredients);
    let missing = difference(&before.measures, &measures_after);
    let added = difference(&measures_after, &before.measures);
    if !missing.is_empty() || !added.is_empty() {
        let mut problem = "The amounts changed.".to_string();
        if !missing.is_empty() {
            problem.push_str(&format!(" Missing now: {}.", missing.join(", ")));
        }
        if !added.is_empty() {
            problem.push_str(&format!(" New: {}.", added.join(", ")));
        }
        return Err(problem);
    }
    // Each line the catalog can name needs a rewritten line of its own that
    // keeps its food and its amounts, so amounts can't trade places between
    // ingredients.
    let after: Vec<Line> = ingredients.iter().map(Line::of).collect();
    let named: Vec<&Line> = before
        .lines
        .iter()
        .filter(|line| !line.foods.is_empty())
        .collect();
    let candidates: Vec<Vec<usize>> = named
        .iter()
        .map(|line| {
            (0..after.len())
                .filter(|&at| after[at].amounts == line.amounts && line.kept_by(&after[at]))
                .collect()
        })
        .collect();
    let matched = match_lines(&candidates, after.len());
    let (mut lost, mut moved) = (Vec::new(), Vec::new());
    for (line, matched) in named.iter().zip(matched) {
        if matched.is_some() {
            continue;
        }
        if after.iter().any(|other| line.kept_by(other)) {
            moved.push(format!(
                "\"{}\"",
                format!("{}: {}", line.food_names(), line.amounts).trim()
            ));
        } else {
            lost.push(line.food_names());
        }
    }
    if !lost.is_empty() {
        lost.sort();
        lost.dedup();
        return Err(format!(
            "These ingredients are gone: {}. Keep every ingredient the recipe started with.",
            lost.join(", ")
        ));
    }
    if !moved.is_empty() {
        return Err(format!(
            "Amounts moved between ingredients; give each of these foods its own amount back: {}.",
            moved.join(", ")
        ));
    }
    Ok(())
}

/// A largest one-to-one matching of original lines to the rewritten lines
/// each could be (`candidates[i]` lists line i's), as each original line's
/// match. Matched by augmenting paths, so one line taking an ambiguous match
/// can't leave another without its only one.
fn match_lines(candidates: &[Vec<usize>], rewritten: usize) -> Vec<Option<usize>> {
    fn augment(
        line: usize,
        candidates: &[Vec<usize>],
        seen: &mut [bool],
        owner: &mut [Option<usize>],
    ) -> bool {
        for &at in &candidates[line] {
            if seen[at] {
                continue;
            }
            seen[at] = true;
            if owner[at].is_none_or(|other| augment(other, candidates, seen, owner)) {
                owner[at] = Some(line);
                return true;
            }
        }
        false
    }
    let mut owner = vec![None; rewritten];
    for line in 0..candidates.len() {
        augment(line, candidates, &mut vec![false; rewritten], &mut owner);
    }
    let mut matched = vec![None; candidates.len()];
    for (at, line) in owner.into_iter().enumerate() {
        if let Some(line) = line {
            matched[line] = Some(at);
        }
    }
    matched
}

/// The amounts in `a` that `b` lacks, counting repeats, as quoted text.
fn difference(a: &[(String, String)], b: &[(String, String)]) -> Vec<String> {
    let mut rest = b.to_vec();
    a.iter()
        .filter(
            |measure| match rest.iter().position(|other| other == *measure) {
                Some(at) => {
                    rest.remove(at);
                    false
                }
                None => true,
            },
        )
        .map(|(amount, unit)| format!("\"{}\"", format!("{amount} {unit}").trim()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::test_support::ScriptedClient;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Serialize, Deserialize, Debug)]
    struct Recipe {
        title: String,
        ingredients: Vec<Value>,
        instructions: String,
    }

    fn recipe() -> Recipe {
        Recipe {
            title: "Onion soup".to_string(),
            ingredients: vec![
                json!({"item": "beef stock", "measurements": [{"amount": "4", "unit": "cup"}], "note": null, "raw": "4 cups beef stock", "section": null}),
                json!({"item": "onion", "measurements": [{"amount": "2", "unit": null}], "note": "sliced", "raw": "2 onions, sliced", "section": null}),
            ],
            instructions: "Cook the onions.\nAdd the stock.".to_string(),
        }
    }

    fn reply_to_finish(client: &ScriptedClient, request: usize) -> String {
        let requests = client.requests.lock().unwrap();
        requests[request]
            .messages
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn moves_prep_into_steps_and_reorders() {
        let client = ScriptedClient::new(vec![
            vec![
                ("set_ingredient", json!({"index": 2, "line": "2 onions"})),
                ("move_ingredient", json!({"index": 2, "after": 0})),
                ("add_step", json!({"after": 0, "text": "Slice the onions."})),
            ],
            vec![(
                "finish",
                json!({"summary": "Moved the slicing into a step."}),
            )],
        ]);

        let result = house_style(&client, &recipe()).await.unwrap();

        assert_eq!(result.recipe.ingredients[0]["item"], "onions");
        assert_eq!(result.recipe.ingredients[1]["item"], "beef stock");
        assert_eq!(
            result.recipe.instructions,
            "Slice the onions.\nCook the onions.\nAdd the stock."
        );
        assert_eq!(result.summary, "Moved the slicing into a step.");
        // Only the ingredient and step tools are offered.
        let requests = client.requests.lock().unwrap();
        let offered: Vec<&str> = requests[0].tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(offered.len(), HOUSE_STYLE_TOOLS.len());
        assert!(!offered.contains(&"scale_recipe"));
        assert!(!offered.contains(&"set_field"));
    }

    #[tokio::test]
    async fn finish_is_refused_until_the_amounts_are_back() {
        let client = ScriptedClient::new(vec![
            vec![(
                "set_ingredient",
                json!({"index": 1, "line": "1 qt beef stock"}),
            )],
            vec![("finish", json!({"summary": "Done."}))],
            vec![(
                "set_ingredient",
                json!({"index": 1, "line": "4 cups beef stock"}),
            )],
            vec![("finish", json!({"summary": "Done."}))],
        ]);

        let result = house_style(&client, &recipe()).await.unwrap();

        assert_eq!(result.turns, 4);
        let refusal = reply_to_finish(&client, 2);
        assert!(
            refusal.starts_with("Error: The amounts changed."),
            "{refusal}"
        );
        assert!(refusal.contains("Missing now: \"4 cup\""), "{refusal}");
    }

    #[tokio::test]
    async fn finish_is_refused_when_a_food_is_dropped() {
        let client = ScriptedClient::new(vec![
            vec![("set_ingredient", json!({"index": 2, "line": "2 shallots"}))],
            vec![("finish", json!({"summary": "Done."}))],
        ]);

        let result = house_style(&client, &recipe()).await;

        assert!(result.is_err());
        let refusal = reply_to_finish(&client, 2);
        assert!(refusal.contains("These ingredients are gone"), "{refusal}");
    }

    #[tokio::test]
    async fn tools_outside_the_set_are_refused() {
        let client = ScriptedClient::new(vec![
            vec![("scale_recipe", json!({"factor": 2}))],
            vec![("finish", json!({"summary": "Nothing to change."}))],
        ]);

        let result = house_style(&client, &recipe()).await.unwrap();

        assert_eq!(
            result.recipe.ingredients[0]["measurements"][0]["amount"],
            "4"
        );
        let reply = reply_to_finish(&client, 1);
        assert_eq!(reply, "Error: there is no tool called scale_recipe");
    }

    #[tokio::test]
    async fn finish_is_refused_when_amounts_swap_between_ingredients() {
        let recipe = Recipe {
            title: "Cake".to_string(),
            ingredients: vec![
                json!({"item": "all-purpose flour", "measurements": [{"amount": "1", "unit": "cup"}], "note": null, "raw": "1 cup all-purpose flour", "section": null}),
                json!({"item": "granulated sugar", "measurements": [{"amount": "2", "unit": "cup"}], "note": null, "raw": "2 cups granulated sugar", "section": null}),
            ],
            instructions: "Mix.".to_string(),
        };
        let client = ScriptedClient::new(vec![
            vec![
                (
                    "set_ingredient",
                    json!({"index": 1, "line": "2 cups all-purpose flour"}),
                ),
                (
                    "set_ingredient",
                    json!({"index": 2, "line": "1 cup granulated sugar"}),
                ),
            ],
            vec![("finish", json!({"summary": "Done."}))],
        ]);

        let result = house_style(&client, &recipe).await;

        assert!(result.is_err());
        let refusal = reply_to_finish(&client, 2);
        assert!(
            refusal.contains("Amounts moved between ingredients"),
            "{refusal}"
        );
        assert!(refusal.contains(": 1 cup\""), "{refusal}");
    }

    /// Run a house-style rewrite of `ingredients` that sets each line in
    /// `lines` (by 1-based index) and finishes, returning the reply to that
    /// `finish`: "Done." when it was accepted.
    async fn finish_after(ingredients: Vec<Value>, lines: &[(usize, &str)]) -> String {
        let recipe = Recipe {
            title: "Recipe".to_string(),
            ingredients,
            instructions: "Cook.".to_string(),
        };
        let edits = lines
            .iter()
            .map(|(index, line)| ("set_ingredient", json!({"index": index, "line": line})))
            .collect();
        let client =
            ScriptedClient::new(vec![edits, vec![("finish", json!({"summary": "Done."}))]]);
        match house_style(&client, &recipe).await {
            Ok(result) => result.summary,
            Err(_) => reply_to_finish(&client, 2),
        }
    }

    #[tokio::test]
    async fn a_weighed_can_can_move_its_size_into_parentheses() {
        // firehouse chili gumbo
        let reply = finish_after(
            vec![
                json!({"item": "diced tomatoes", "measurements": [{"amount": "2", "unit": "14.5-ounce cans"}], "note": null, "section": null}),
                json!({"item": "tomato paste", "measurements": [{"amount": "2", "unit": "6-ounce cans"}], "note": null, "section": null}),
            ],
            &[
                (1, "2 (14.5 oz) cans diced tomatoes"),
                (2, "2 (6 oz) cans tomato paste"),
            ],
        )
        .await;
        assert_eq!(reply, "Done.");

        // The size still has to be the same.
        let reply = finish_after(
            vec![json!({"item": "diced tomatoes", "measurements": [{"amount": "2", "unit": "14.5-ounce cans"}], "note": null, "section": null})],
            &[(1, "2 (15 oz) cans diced tomatoes")],
        )
        .await;
        assert!(reply.contains("Missing now: \"14.5 oz\""), "{reply}");
    }

    #[tokio::test]
    async fn dropping_prep_words_keeps_the_food() {
        // sparerib oden: "hardboiled eggs" is a different catalog entry than
        // "eggs", but the same family of food.
        let reply = finish_after(
            vec![json!({"item": "hardboiled eggs", "measurements": [{"amount": "6", "unit": null}], "note": null, "section": null})],
            &[(1, "6 eggs")],
        )
        .await;
        assert_eq!(reply, "Done.");

        // fried egg sandwich: the catalog reads the original line as endive
        // and doesn't know frisée without the item's "endive".
        let reply = finish_after(
            vec![json!({"item": "frisée, torn into-bite size pieces", "measurements": [{"amount": "1.5", "unit": "cup"}, {"amount": "2", "unit": "oz"}], "note": "French curly endive, about 2 ounces", "section": null})],
            &[(1, "1 1/2 cups (2 oz) frisée, French curly endive")],
        )
        .await;
        assert_eq!(reply, "Done.");
    }

    #[tokio::test]
    async fn dropping_a_word_that_names_the_food_is_refused() {
        let reply = finish_after(
            vec![json!({"item": "garlic powder", "measurements": [{"amount": "1", "unit": "tsp"}], "note": null, "section": null})],
            &[(1, "1 tsp garlic")],
        )
        .await;
        assert!(reply.contains("These ingredients are gone"), "{reply}");

        let reply = finish_after(
            vec![json!({"item": "garlic powder", "measurements": [{"amount": "1", "unit": "tsp"}], "note": null, "section": null})],
            &[(1, "1 tsp powder")],
        )
        .await;
        assert!(reply.contains("These ingredients are gone"), "{reply}");
        let reply = finish_after(
            vec![json!({"item": "sweetened condensed milk", "measurements": [{"amount": "1", "unit": "cup"}], "note": null, "section": null})],
            &[(1, "1 cup milk")],
        )
        .await;
        assert!(reply.contains("These ingredients are gone"), "{reply}");

        // Nor can the food go altogether.
        let reply = finish_after(
            vec![json!({"item": "hardboiled eggs", "measurements": [{"amount": "6", "unit": null}], "note": null, "section": null})],
            &[(1, "6")],
        )
        .await;
        assert!(reply.contains("These ingredients are gone"), "{reply}");

        // Nor can a new word turn a food into another of its family.
        let reply = finish_after(
            vec![json!({"item": "eggs", "measurements": [{"amount": "6", "unit": null}], "note": null, "section": null})],
            &[(1, "6 hardboiled eggs")],
        )
        .await;
        assert!(reply.contains("These ingredients are gone"), "{reply}");
    }

    #[tokio::test]
    async fn lines_that_could_match_either_way_still_reorder() {
        // "eggs" could keep either line's food; the hardboiled eggs still
        // find their own line.
        let reply = finish_after(
            vec![
                json!({"item": "hardboiled eggs", "measurements": [{"amount": "6", "unit": null}], "note": null, "section": null}),
                json!({"item": "eggs", "measurements": [{"amount": "6", "unit": null}], "note": null, "section": null}),
            ],
            &[(1, "6 eggs"), (2, "6 hardboiled eggs")],
        )
        .await;
        assert_eq!(reply, "Done.");
    }

    #[test]
    fn measures_compare_parsed_amounts_in_any_order() {
        let a = measures(&[
            json!({"measurements": [{"amount": "1/2", "unit": "Cup"}]}),
            json!({"measurements": [{"amount": "2", "unit": null}]}),
        ]);
        let b = measures(&[
            json!({"measurements": [{"amount": "2", "unit": null}]}),
            json!({"measurements": [{"amount": "0.5", "unit": "cup"}]}),
        ]);
        assert_eq!(a, b);

        let range = |amount: &str| {
            measures(&[json!({"measurements": [{"amount": amount, "unit": "cup"}]})])
        };
        assert_eq!(range("4 to 6"), range("4-6"));
        assert_ne!(range("4 to 6"), range("4"));
    }
}
