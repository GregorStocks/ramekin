//! House style: rewrite a recipe's ingredients and steps to the owner's style
//! guide (`doc/house-style.md`) with the Edit with AI tool loop, restricted to
//! the tools that touch ingredients and steps.
//!
//! `finish` is accepted only once every amount and every catalog food the
//! recipe started with is still there, so a rewrite can reword, reorder and
//! move prep into steps but can't change what the recipe makes.

use std::collections::BTreeSet;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use crate::ai::edit_recipe::{run_edit_loop, EditPrompt};
use crate::ai::edit_recipe_tools::{edit_recipe_tools, format_amount, RecipeDraft, FINISH_TOOL};
use crate::ai::prompts::house_style::{
    render_house_style_system_prompt, render_house_style_user_prompt, HOUSE_STYLE_PROMPT_NAME,
};
use crate::ai::{AiClient, AiError, EditRecipeResult, NoUserData};
use crate::catalog::{self, Resolution};
use crate::metric_weights::parse_amount;

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
    let original = RecipeDraft::new(value).map_err(AiError::ParseError)?;
    let measures = measures(original.ingredients());
    let foods = foods(original.ingredients());

    let tools = edit_recipe_tools()
        .into_iter()
        .filter(|tool| HOUSE_STYLE_TOOLS.contains(&tool.name.as_str()))
        .collect();
    let check = move |draft: &RecipeDraft| check_unchanged(&measures, &foods, draft.ingredients());
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
/// doesn't count as a change. An amount that parses compares as a number
/// ("1/2" and "0.5" are the same).
fn measures(ingredients: &[Value]) -> Vec<(String, String)> {
    let mut measures: Vec<(String, String)> = ingredients
        .iter()
        .flat_map(|ingredient| {
            ingredient
                .get("measurements")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|m| {
            let text = |key: &str| {
                m.get(key)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_lowercase()
            };
            let (amount, unit) = (text("amount"), text("unit"));
            if amount.is_empty() && unit.is_empty() {
                return None;
            }
            let amount = parse_amount(&amount).map_or(amount, format_amount);
            Some((amount, unit))
        })
        .collect();
    measures.sort();
    measures
}

/// The catalog foods the ingredient lines name.
fn foods(ingredients: &[Value]) -> BTreeSet<String> {
    ingredients
        .iter()
        .flat_map(|ingredient| {
            let item = ingredient.get("item").and_then(Value::as_str).unwrap_or("");
            let note = ingredient.get("note").and_then(Value::as_str);
            match catalog::resolve_line(item, note) {
                Resolution::Entry { entry, .. } => vec![entry.id.clone()],
                Resolution::Compound(entries) => {
                    entries.into_iter().map(|entry| entry.id.clone()).collect()
                }
                _ => Vec::new(),
            }
        })
        .collect()
}

/// Whether the rewritten `ingredients` keep the original amounts and foods;
/// the error says what's missing, for the model to fix.
fn check_unchanged(
    measures_before: &[(String, String)],
    foods_before: &BTreeSet<String>,
    ingredients: &[Value],
) -> Result<(), String> {
    let measures_after = measures(ingredients);
    if measures_after != measures_before {
        let missing: Vec<String> = difference(measures_before, &measures_after);
        let added: Vec<String> = difference(&measures_after, measures_before);
        let mut problem = "The amounts changed.".to_string();
        if !missing.is_empty() {
            problem.push_str(&format!(" Missing now: {}.", missing.join(", ")));
        }
        if !added.is_empty() {
            problem.push_str(&format!(" New: {}.", added.join(", ")));
        }
        return Err(problem);
    }
    let foods_after = foods(ingredients);
    let lost: Vec<&str> = foods_before
        .difference(&foods_after)
        .map(String::as_str)
        .collect();
    if !lost.is_empty() {
        return Err(format!(
            "These ingredients are gone: {}. Keep every ingredient the recipe started with.",
            lost.join(", ")
        ));
    }
    Ok(())
}

/// The measures in `a` that `b` lacks, counting repeats, as text.
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
    use crate::ai::{ChatMessage, ChatRequest, ChatResponse, ToolCall, Usage};
    use async_trait::async_trait;
    use serde::Deserialize;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// Answers each turn with the next scripted tool calls, and records the
    /// requests it saw.
    struct ScriptedClient {
        turns: Mutex<VecDeque<Vec<(&'static str, Value)>>>,
        requests: Mutex<Vec<ChatRequest>>,
    }

    impl ScriptedClient {
        fn new(turns: Vec<Vec<(&'static str, Value)>>) -> Self {
            Self {
                turns: Mutex::new(turns.into()),
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl AiClient for ScriptedClient {
        async fn complete(
            &self,
            _prompt_name: &str,
            request: &ChatRequest,
        ) -> Result<ChatResponse, AiError> {
            self.requests.lock().unwrap().push(request.clone());
            let calls = self.turns.lock().unwrap().pop_front().unwrap_or_default();
            Ok(ChatResponse {
                content: String::new(),
                usage: Usage::default(),
                cached: false,
                tool_calls: calls
                    .into_iter()
                    .enumerate()
                    .map(|(i, (name, args))| ToolCall {
                        id: format!("call-{i}"),
                        name: name.to_string(),
                        arguments: args.to_string(),
                    })
                    .collect(),
            })
        }

        fn forget(&self, _prompt_name: &str, _messages: &[ChatMessage]) {}
    }

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
    }
}
