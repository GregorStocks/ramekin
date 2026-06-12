//! ParseIngredients step - parses raw ingredient strings into structured data
//! and enriches them with metric weight and volume-to-weight alternatives.

use std::time::Instant;

use async_trait::async_trait;

use crate::ingredient_parser::{parse_ingredients, ParsedIngredient};
use crate::metric_weights::{add_metric_weight_alternative, MetricConversionStats};
use crate::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};
use crate::types::{ParseIngredientsOutput, RawRecipe};
use crate::volume_to_weight::{
    add_volume_to_weight_alternative, apply_ingredient_rewrites, VolumeConversionStats,
};

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
        let mut parsed = parse_ingredients(&raw_recipe.ingredients);

        // Annotate ingredients with footnote text extracted from HTML
        if let Some(ref footnotes) = raw_recipe.footnotes {
            apply_footnotes_to_ingredients(&mut parsed, footnotes);
        }

        // Enrich with metric weight alternatives (oz/lb → g)
        let mut weight_stats = MetricConversionStats::default();
        // Enrich with volume-to-weight alternatives for known ingredients
        let mut volume_stats = VolumeConversionStats::default();
        let enriched: Vec<_> = parsed
            .into_iter()
            .map(apply_ingredient_rewrites)
            .map(|ing| add_metric_weight_alternative(ing, &mut weight_stats))
            .map(|ing| add_volume_to_weight_alternative(ing, &mut volume_stats))
            .map(|ing| ing.normalize_amounts())
            .collect();

        let output = ParseIngredientsOutput {
            ingredients: enriched,
            volume_stats: Some(volume_stats),
            metric_stats: Some(weight_stats),
        };

        StepResult {
            step_name: Self::NAME.to_string(),
            success: true,
            output: serde_json::to_value(&output)
                .expect("ParseIngredientsOutput serializes to JSON"),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: Some("save_recipe".to_string()),
        }
    }
}

/// Apply extracted footnote text to ingredients that have asterisk markers.
///
/// For each ingredient, checks its `raw` text for trailing asterisks and matches
/// the marker count to the footnotes list. When a match is found, the footnote
/// text is appended to the ingredient's `note` field.
fn apply_footnotes_to_ingredients(
    ingredients: &mut [ParsedIngredient],
    footnotes: &[(String, String)],
) {
    if footnotes.is_empty() {
        return;
    }

    let footnote_map: std::collections::HashMap<&str, &str> = footnotes
        .iter()
        .map(|(marker, text)| (marker.as_str(), text.as_str()))
        .collect();

    for ingredient in ingredients.iter_mut() {
        // Use raw text (not item) because the parser already stripped asterisks from item
        let raw = match ingredient.raw.as_deref() {
            Some(r) => r,
            None => continue,
        };

        let marker = extract_trailing_marker(raw.trim());
        if marker.is_empty() {
            continue;
        }

        if let Some(&footnote_text) = footnote_map.get(marker.as_str()) {
            ingredient.note = match &ingredient.note {
                Some(existing) => Some(format!("{}; {}", existing, footnote_text)),
                None => Some(footnote_text.to_string()),
            };
        }
    }
}

/// Extract a footnote asterisk marker from an ingredient string.
/// Finds runs of 1-3 `*` characters that are followed by a word boundary
/// (whitespace, comma, paren, end of string) — the pattern recipe sites
/// use for footnote references.
fn extract_trailing_marker(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'*' {
            let start = i;
            while i < bytes.len() && bytes[i] == b'*' {
                i += 1;
            }
            let count = i - start;
            // Valid marker: 1-3 asterisks followed by end-of-string or a boundary char
            if (1..=3).contains(&count)
                && (i >= bytes.len()
                    || bytes[i] == b' '
                    || bytes[i] == b','
                    || bytes[i] == b'('
                    || bytes[i] == b')')
            {
                return "*".repeat(count);
            }
        } else {
            i += 1;
        }
    }
    String::new()
}
