//! Edit with AI: a bounded tool-calling loop that changes a recipe the way the
//! user describes.
//!
//! The model sees the recipe with its ingredients and steps numbered, looks
//! things up (catalog weights and calories, the user's tags and recipes),
//! makes targeted edits, and calls `finish` with a summary. Each turn goes
//! through the caching client with the whole conversation so far, so a rerun
//! of the same edit replays from the cache.

use serde::{de::DeserializeOwned, Serialize};

use crate::ai::edit_recipe_tools::{
    edit_recipe_tools, run_tool, RecipeDraft, RecipeEditContext, FINISH_TOOL,
};
use crate::ai::prompts::edit_recipe::{
    render_edit_recipe_system_prompt, render_edit_recipe_user_prompt, EDIT_RECIPE_PROMPT_NAME,
};
use crate::ai::{AiClient, AiError, ChatMessage, ChatRequest, ImageData, ToolSpec, Usage};

/// The most model calls one edit makes before giving up.
pub const MAX_EDIT_TURNS: usize = 20;
/// The most tool calls one edit makes before giving up.
pub const MAX_EDIT_TOOL_CALLS: usize = 60;

/// Result of an AI recipe edit.
pub struct EditRecipeResult<T> {
    /// The edited recipe parsed into the caller's shape.
    pub recipe: T,
    /// What the model says it changed, for the user.
    pub summary: String,
    /// Whether every turn came from the cache.
    pub cached: bool,
    /// Tokens across every turn.
    pub usage: Usage,
    /// Model calls made.
    pub turns: usize,
}

/// Change `recipe` as `instruction` says, with `images` of the dish as
/// context. `recipe` serializes to a JSON object with the server's recipe
/// fields (title, ingredients, instructions…); the edited object must parse
/// back into `T`.
pub async fn edit_recipe<T: Serialize + DeserializeOwned>(
    ai_client: &dyn AiClient,
    recipe: &T,
    instruction: &str,
    images: Vec<ImageData>,
    context: &dyn RecipeEditContext,
) -> Result<EditRecipeResult<T>, AiError> {
    run_edit_loop(
        ai_client,
        recipe,
        EditPrompt {
            name: EDIT_RECIPE_PROMPT_NAME,
            system: render_edit_recipe_system_prompt(),
            tools: edit_recipe_tools(),
            check: None,
        },
        |rendered| render_edit_recipe_user_prompt(rendered, instruction),
        images,
        context,
    )
    .await
}

/// What a tool-loop run asks of the model.
pub(crate) struct EditPrompt {
    /// Prompt name for cache keys.
    pub name: &'static str,
    pub system: String,
    /// The tools the model may call; any other is refused.
    pub tools: Vec<ToolSpec>,
    /// What must hold of the edited recipe before `finish` is accepted; its
    /// error goes back to the model to fix.
    pub check: Option<DraftCheck>,
}

/// A check of the edited recipe, returning what's wrong.
pub(crate) type DraftCheck = Box<dyn Fn(&RecipeDraft) -> Result<(), String> + Send + Sync>;

/// Run the edit tool loop on `recipe`: `user_prompt` turns the rendered
/// recipe into the first user message.
pub(crate) async fn run_edit_loop<T: Serialize + DeserializeOwned>(
    ai_client: &dyn AiClient,
    recipe: &T,
    prompt: EditPrompt,
    user_prompt: impl FnOnce(&str) -> String,
    images: Vec<ImageData>,
    context: &dyn RecipeEditContext,
) -> Result<EditRecipeResult<T>, AiError> {
    let value = serde_json::to_value(recipe)
        .map_err(|e| AiError::ParseError(format!("Recipe doesn't serialize: {e}")))?;
    let mut draft = RecipeDraft::new(value).map_err(AiError::ParseError)?;

    let user_prompt = user_prompt(&draft.render());
    let user_message = if images.is_empty() {
        ChatMessage::user(user_prompt)
    } else {
        ChatMessage::user_with_images(user_prompt, images)
    };
    let mut request = ChatRequest {
        messages: vec![ChatMessage::system(prompt.system), user_message],
        // A turn is a few tool calls, after the model's hidden reasoning.
        max_tokens: Some(8192),
        temperature: Some(0.7),
        tools: prompt.tools,
        ..Default::default()
    };

    let mut usage = Usage::default();
    let mut cached = true;
    let mut tool_calls = 0;
    for turn in 1..=MAX_EDIT_TURNS {
        let response = ai_client.complete(prompt.name, &request).await?;
        usage.prompt_tokens += response.usage.prompt_tokens;
        usage.completion_tokens += response.usage.completion_tokens;
        usage.total_tokens += response.usage.total_tokens;
        cached &= response.cached;

        if response.tool_calls.is_empty() {
            // An answer in prose: ask for the tools instead.
            request
                .messages
                .push(ChatMessage::assistant(response.content));
            request.messages.push(ChatMessage::user(
                "Make the changes with the edit tools, then call finish with a summary.",
            ));
            continue;
        }

        request.messages.push(ChatMessage::assistant_tool_calls(
            response.content,
            response.tool_calls.clone(),
        ));
        let mut summary = None;
        for call in &response.tool_calls {
            tool_calls += 1;
            if tool_calls > MAX_EDIT_TOOL_CALLS {
                return Err(AiError::Api(format!(
                    "The edit made more than {MAX_EDIT_TOOL_CALLS} tool calls without finishing"
                )));
            }
            let reply = if call.name == FINISH_TOOL {
                match prompt.check.as_ref().map_or(Ok(()), |check| check(&draft)) {
                    Ok(()) => {
                        let text = serde_json::from_str::<serde_json::Value>(&call.arguments)
                            .ok()
                            .and_then(|args| args["summary"].as_str().map(str::to_string))
                            .unwrap_or_default();
                        summary = Some(text);
                        "Done.".to_string()
                    }
                    Err(problem) => {
                        format!("Error: {problem} Fix that, then call finish again.")
                    }
                }
            } else if !request.tools.iter().any(|tool| tool.name == call.name) {
                format!("Error: there is no tool called {}", call.name)
            } else {
                run_tool(&mut draft, context, &call.name, &call.arguments).await
            };
            request
                .messages
                .push(ChatMessage::tool_result(call.id.clone(), reply));
        }

        if let Some(summary) = summary {
            let recipe = serde_json::from_value(draft.into_value())
                .map_err(|e| AiError::ParseError(format!("The edited recipe doesn't fit: {e}")))?;
            return Ok(EditRecipeResult {
                recipe,
                summary,
                cached,
                usage,
                turns: turn,
            });
        }
    }
    Err(AiError::Api(format!(
        "The edit didn't finish within {MAX_EDIT_TURNS} turns"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::edit_recipe_tools::NoUserData;
    use crate::ai::test_support::ScriptedClient;
    use crate::ai::Role;
    use serde::Deserialize;

    #[derive(Serialize, Deserialize, Debug)]
    struct Recipe {
        title: String,
        ingredients: Vec<serde_json::Value>,
        instructions: String,
        servings: Option<String>,
    }

    fn recipe() -> Recipe {
        Recipe {
            title: "Pancakes".to_string(),
            ingredients: vec![serde_json::json!({
                "item": "flour", "measurements": [{"amount": "1", "unit": "cup"}],
                "note": null, "raw": "1 cup flour", "section": null
            })],
            instructions: "Mix.\nCook.".to_string(),
            servings: Some("2".to_string()),
        }
    }

    #[tokio::test]
    async fn runs_tools_until_finish_and_feeds_results_back() {
        let client = ScriptedClient::new(vec![
            vec![("lookup_ingredient", serde_json::json!({"name": "flour"}))],
            vec![
                ("scale_recipe", serde_json::json!({"factor": 2})),
                (
                    "set_field",
                    serde_json::json!({"field": "servings", "value": "4"}),
                ),
            ],
            vec![("finish", serde_json::json!({"summary": "Doubled it."}))],
        ]);

        let result = edit_recipe(&client, &recipe(), "double it", vec![], &NoUserData)
            .await
            .unwrap();

        assert_eq!(result.summary, "Doubled it.");
        assert_eq!(result.turns, 3);
        assert_eq!(result.usage.total_tokens, 45);
        assert_eq!(result.recipe.servings.as_deref(), Some("4"));
        assert_eq!(
            result.recipe.ingredients[0]["measurements"][0]["amount"],
            "2"
        );
        assert_eq!(result.recipe.instructions, "Mix.\nCook.");

        let requests = client.requests.lock().unwrap();
        // The last request replays both earlier turns and their results.
        let last = &requests[2].messages;
        let tool_results: Vec<_> = last.iter().filter(|m| m.role == Role::Tool).collect();
        assert_eq!(tool_results.len(), 3);
        assert_eq!(tool_results[0].tool_call_id.as_deref(), Some("call-0-0"));
        assert!(tool_results[1].content.starts_with("Scaled 1 amounts by 2"));
        assert!(!requests[0].tools.is_empty());
    }

    #[tokio::test]
    async fn gives_up_after_the_turn_cap() {
        let client = ScriptedClient::new(
            (0..MAX_EDIT_TURNS)
                .map(|_| vec![("list_tags", serde_json::json!({}))])
                .collect(),
        );

        let result = edit_recipe(&client, &recipe(), "tag it", vec![], &NoUserData).await;

        assert!(matches!(result, Err(AiError::Api(message)) if message.contains("didn't finish")));
    }

    #[tokio::test]
    async fn prose_answers_are_nudged_back_to_the_tools() {
        let client = ScriptedClient::new(vec![
            vec![],
            vec![(
                "finish",
                serde_json::json!({"summary": "Nothing to change."}),
            )],
        ]);

        let result = edit_recipe(&client, &recipe(), "hmm", vec![], &NoUserData)
            .await
            .unwrap();

        assert_eq!(result.turns, 2);
        assert_eq!(result.recipe.title, "Pancakes");
    }
}
