//! House style step - rewrite ingredients and steps to the owner's style
//! guide.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::ai::{house_style, AiClient, Usage};
use crate::ingredient_parser::ParsedIngredient;
use crate::pipeline::steps::{base_raw_recipe, step_failure, ParseIngredientsStep};
use crate::pipeline::{
    deserialize_required_output_field, PipelineStep, StepContext, StepMetadata, StepResult,
    LOAD_RECIPE_STEP,
};

/// Step that rewrites a recipe's ingredients and steps into the house style.
pub struct HouseStyleStep {
    ai_client: Arc<dyn AiClient>,
}

impl HouseStyleStep {
    pub const NAME: &'static str = "house_style";

    /// `ai_client` should be configured for recipe edits
    /// (`AiConfig::for_recipe_edit`): the rewrite is a tool loop.
    pub fn new(ai_client: Arc<dyn AiClient>) -> Self {
        Self { ai_client }
    }
}

/// What the model sees and rewrites.
#[derive(Debug, Serialize, Deserialize)]
struct StyledRecipe {
    title: String,
    ingredients: Vec<ParsedIngredient>,
    instructions: String,
}

#[derive(Debug, Serialize)]
struct HouseStyleOutput {
    ingredients: Vec<ParsedIngredient>,
    instructions: String,
    summary: String,
    changed: bool,
    cached: bool,
    usage: Usage,
}

/// The structured ingredients the job's recipe has: freshly parsed when it
/// read the source, or the loaded version's.
async fn base_ingredients(ctx: &StepContext<'_>) -> Result<Vec<ParsedIngredient>, String> {
    for step_name in [ParseIngredientsStep::NAME, LOAD_RECIPE_STEP] {
        if let Some(output) = ctx.outputs.get_output(step_name).await {
            return deserialize_required_output_field(&output, step_name, "ingredients");
        }
    }
    Err(format!(
        "No {LOAD_RECIPE_STEP} or {} output found",
        ParseIngredientsStep::NAME
    ))
}

#[async_trait]
impl PipelineStep for HouseStyleStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Rewrite the recipe in house style",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();
        let raw_recipe = match base_raw_recipe(ctx).await {
            Ok(r) => r,
            Err(e) => return step_failure(Self::NAME, start, e),
        };
        let ingredients = match base_ingredients(ctx).await {
            Ok(ingredients) => ingredients,
            Err(e) => return step_failure(Self::NAME, start, e),
        };
        let recipe = StyledRecipe {
            title: raw_recipe.title,
            ingredients,
            instructions: raw_recipe.instructions,
        };

        let result = match house_style(self.ai_client.as_ref(), &recipe).await {
            Ok(r) => r,
            Err(e) => return step_failure(Self::NAME, start, format!("AI call failed: {e}")),
        };

        let styled = result.recipe;
        let output = HouseStyleOutput {
            changed: styled.ingredients != recipe.ingredients
                || styled.instructions != recipe.instructions,
            ingredients: styled.ingredients,
            instructions: styled.instructions,
            summary: result.summary,
            cached: result.cached,
            usage: result.usage,
        };

        StepResult {
            step_name: Self::NAME.to_string(),
            success: true,
            output: serde_json::to_value(output).expect("HouseStyleOutput serializes to JSON"),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};
    use std::error::Error;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use serde_json::{json, Value as JsonValue};

    use super::HouseStyleStep;
    use crate::ai::{
        AiClient, AiError, ChatMessage, ChatRequest, ChatResponse, ConfigError, ToolCall,
        UnconfiguredAiClient, Usage,
    };
    use crate::pipeline::{PipelineStep, StepContext, StepOutputStore};

    #[derive(Default)]
    struct TestOutputStore {
        outputs: HashMap<String, JsonValue>,
    }

    #[async_trait]
    impl StepOutputStore for TestOutputStore {
        async fn get_output(&self, step_name: &str) -> Option<JsonValue> {
            self.outputs.get(step_name).cloned()
        }

        async fn save_output(
            &mut self,
            _step_name: &str,
            _output: &JsonValue,
            _duration_ms: i64,
            _success: bool,
            _error: Option<&str>,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            Ok(())
        }
    }

    /// Answers each turn with the next scripted tool calls.
    struct ScriptedClient(Mutex<VecDeque<Vec<(&'static str, JsonValue)>>>);

    #[async_trait]
    impl AiClient for ScriptedClient {
        async fn complete(
            &self,
            _prompt_name: &str,
            _request: &ChatRequest,
        ) -> Result<ChatResponse, AiError> {
            let calls = self.0.lock().unwrap().pop_front().unwrap_or_default();
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

    /// A manual run: the loaded version, with its structured ingredients.
    fn loaded() -> TestOutputStore {
        let mut outputs = TestOutputStore::default();
        outputs.outputs.insert(
            "load_recipe".to_string(),
            json!({
                "raw_recipe": {
                    "title": "Soup",
                    "ingredients": "[For the soup]\n2 onions, sliced",
                    "instructions": "Cook the onions.",
                    "image_urls": [],
                },
                "ingredients": [{
                    "item": "onions",
                    "measurements": [{"amount": "2", "unit": null}],
                    "note": "sliced",
                    "raw": null,
                    "section": "For the soup",
                }],
            }),
        );
        outputs
    }

    #[tokio::test]
    async fn rewrites_the_loaded_version() {
        let outputs = loaded();
        let ctx = StepContext {
            url: "https://example.test/soup",
            outputs: &outputs,
        };
        let client = ScriptedClient(Mutex::new(VecDeque::from([
            vec![
                ("set_ingredient", json!({"index": 1, "line": "2 onions"})),
                ("add_step", json!({"after": 0, "text": "Slice the onions."})),
            ],
            vec![("finish", json!({"summary": "Moved the slicing."}))],
        ])));

        let result = HouseStyleStep::new(Arc::new(client)).execute(&ctx).await;

        assert!(result.success, "{:?}", result.error);
        let output = result.output;
        assert_eq!(output["changed"], true);
        assert_eq!(output["summary"], "Moved the slicing.");
        assert_eq!(output["ingredients"][0]["note"], JsonValue::Null);
        assert_eq!(output["ingredients"][0]["section"], "For the soup");
        assert_eq!(
            output["instructions"],
            "Slice the onions.\nCook the onions."
        );
    }

    #[tokio::test]
    async fn a_recipe_already_in_style_is_unchanged() {
        let outputs = loaded();
        let ctx = StepContext {
            url: "https://example.test/soup",
            outputs: &outputs,
        };
        let client = ScriptedClient(Mutex::new(VecDeque::from([vec![(
            "finish",
            json!({"summary": "Already in house style."}),
        )]])));

        let result = HouseStyleStep::new(Arc::new(client)).execute(&ctx).await;

        assert!(result.success, "{:?}", result.error);
        assert_eq!(result.output["changed"], false);
    }

    #[tokio::test]
    async fn unconfigured_ai_client_fails_step() {
        let outputs = loaded();
        let ctx = StepContext {
            url: "https://example.test/soup",
            outputs: &outputs,
        };

        let step = HouseStyleStep::new(Arc::new(UnconfiguredAiClient::new(
            ConfigError::MissingEnvVar("OPENROUTER_API_KEY".to_string()),
        )));
        let result = step.execute(&ctx).await;

        assert!(!result.success);
        assert!(result.error.unwrap().contains("OPENROUTER_API_KEY"));
    }
}
