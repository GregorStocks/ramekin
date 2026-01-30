//! Enrich step - add metric weight alternatives to ingredients.
//!
//! This step converts oz measurements to grams, adding the metric
//! equivalent as an alternative measurement.

use std::time::Instant;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::json;

use crate::metric_weights::{add_metric_weight_alternative, EnrichmentStats};
use crate::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};
use crate::types::ParseIngredientsOutput;

/// Step that adds metric weight alternatives to ingredients.
///
/// This step is fully deterministic - no AI involved.
/// Currently converts oz → grams.
pub struct EnrichMetricWeightsStep;

impl EnrichMetricWeightsStep {
    /// Step name constant.
    pub const NAME: &'static str = "enrich_metric_weights";
}

/// Output of the metric weights enrichment step.
#[derive(Debug, Serialize)]
pub struct MetricWeightsOutput {
    pub ingredients: Vec<crate::ingredient_parser::ParsedIngredient>,
    pub stats: MetricWeightsStats,
}

/// Serializable stats for the step output.
#[derive(Debug, Serialize)]
pub struct MetricWeightsStats {
    pub converted: usize,
    pub skipped_no_oz: usize,
    pub skipped_already_metric: usize,
    pub skipped_unparseable: usize,
}

impl From<EnrichmentStats> for MetricWeightsStats {
    fn from(stats: EnrichmentStats) -> Self {
        Self {
            converted: stats.converted,
            skipped_no_oz: stats.skipped_no_oz,
            skipped_already_metric: stats.skipped_already_metric,
            skipped_unparseable: stats.skipped_unparseable,
        }
    }
}

#[async_trait]
impl PipelineStep for EnrichMetricWeightsStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Add metric weight alternatives to ingredients",
            continues_on_failure: true, // Don't fail pipeline if enrichment fails
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Get parsed ingredients from parse_ingredients output
        let parse_output = match ctx.outputs.get_output("parse_ingredients") {
            Some(o) => o,
            None => {
                return StepResult {
                    step_name: Self::NAME.to_string(),
                    success: false,
                    output: json!({ "error": "No parse_ingredients output found" }),
                    error: Some("No parse_ingredients output found".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: Some("save_recipe".to_string()),
                };
            }
        };

        let ingredients_output: ParseIngredientsOutput =
            match serde_json::from_value(parse_output.clone()) {
                Ok(o) => o,
                Err(e) => {
                    return StepResult {
                        step_name: Self::NAME.to_string(),
                        success: false,
                        output: json!({ "error": format!("Failed to parse ingredients: {}", e) }),
                        error: Some(format!("Failed to parse ingredients: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        next_step: Some("save_recipe".to_string()),
                    };
                }
            };

        // Enrich each ingredient
        let mut stats = EnrichmentStats::default();
        let enriched: Vec<_> = ingredients_output
            .ingredients
            .into_iter()
            .map(|ing| add_metric_weight_alternative(ing, &mut stats))
            .collect();

        let output = MetricWeightsOutput {
            ingredients: enriched,
            stats: stats.into(),
        };

        StepResult {
            step_name: Self::NAME.to_string(),
            success: true,
            output: serde_json::to_value(&output).unwrap_or(json!({})),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: Some("save_recipe".to_string()),
        }
    }
}
