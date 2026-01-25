//! ExtractRecipe step - extracts recipe data from HTML.

use std::time::Instant;

use async_trait::async_trait;

use crate::extract::extract_recipe_with_stats;
use crate::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};

/// Step that extracts recipe data from HTML using JSON-LD or microdata.
pub struct ExtractRecipeStep;

impl ExtractRecipeStep {
    /// Step name constant.
    pub const NAME: &'static str = "extract_recipe";
}

#[async_trait]
impl PipelineStep for ExtractRecipeStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Extract recipe from HTML",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Get HTML from prior step (duck typing - we know fetch_html outputs "html")
        let fetch_output = match ctx.outputs.get_output("fetch_html") {
            Some(o) => o,
            None => {
                return StepResult {
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some("fetch_html output not found".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        let html = match fetch_output.get("html").and_then(|v| v.as_str()) {
            Some(h) => h,
            None => {
                return StepResult {
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some("No 'html' field in fetch_html output".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        match extract_recipe_with_stats(html, ctx.url) {
            Ok(output) => StepResult {
                success: true,
                output: serde_json::to_value(&output).unwrap_or_default(),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: Some("fetch_images".to_string()),
            },
            Err(e) => StepResult {
                success: false,
                output: serde_json::Value::Null,
                error: Some(e.to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: None,
            },
        }
    }
}
