//! ParseIngredients step - parses raw ingredient strings into structured data
//! and enriches them with metric weight alternatives.

use std::time::Instant;

use async_trait::async_trait;

use crate::ingredient_parser::parse_ingredients;
use crate::metric_weights::{add_metric_weight_alternative, EnrichmentStats};
use crate::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};
use crate::types::{ParseIngredientsOutput, RawRecipe};

/// Step that parses raw ingredient strings into structured data.
///
/// This step reads the raw ingredient blob from extract_recipe output and
/// parses each line into structured Ingredient data with amounts, units,
/// and preparation notes extracted.
pub struct ParseIngredientsStep;

impl ParseIngredientsStep {
    /// Step name constant.
    pub const NAME: &'static str = "parse_ingredients";
}

#[async_trait]
impl PipelineStep for ParseIngredientsStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Parse ingredient strings into structured data",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Get extract output to find raw ingredients
        let extract_output = match ctx.outputs.get_output("extract_recipe") {
            Some(o) => o,
            None => {
                return StepResult {
                    step_name: Self::NAME.to_string(),
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some("extract_recipe output not found".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Parse raw_recipe to get ingredients blob
        let raw_recipe: RawRecipe = match extract_output
            .get("raw_recipe")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
        {
            Some(r) => r,
            None => {
                return StepResult {
                    step_name: Self::NAME.to_string(),
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some("No raw_recipe in extract output".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Parse the ingredients blob into structured data
        let parsed = parse_ingredients(&raw_recipe.ingredients);

        // Enrich with metric weight alternatives (oz → g)
        let mut stats = EnrichmentStats::default();
        let enriched: Vec<_> = parsed
            .into_iter()
            .map(|ing| add_metric_weight_alternative(ing, &mut stats))
            .collect();

        let output = ParseIngredientsOutput {
            ingredients: enriched,
        };

        StepResult {
            step_name: Self::NAME.to_string(),
            success: true,
            output: serde_json::to_value(&output).unwrap_or_default(),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: Some("save_recipe".to_string()),
        }
    }
}
