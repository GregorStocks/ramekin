//! ParseIngredients step - parses raw ingredient strings into structured data.
//! Gram amounts are computed when a recipe is read, never stored; this step
//! only counts which lines would get one.

use std::time::Instant;

use async_trait::async_trait;

use crate::catalog::Learned;
use crate::ingredient_parser::{parse_ingredients, parse_paprika_ingredients, ParsedIngredient};
use crate::metric_weights::MetricConversionStats;
use crate::pipeline::{
    deserialize_required_output_field, PipelineStep, StepContext, StepMetadata, StepResult,
};
use crate::types::{ExtractionMethod, ParseIngredientsOutput, RawRecipe};
use crate::volume_to_weight::{derived_grams_with_stats, VolumeConversionStats};

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
        let extract_output = match ctx.outputs.get_output("extract_recipe").await {
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
        let raw_recipe: RawRecipe = match deserialize_required_output_field(
            &extract_output,
            "extract_recipe",
            "raw_recipe",
        ) {
            Ok(r) => r,
            Err(e) => {
                return StepResult {
                    step_name: Self::NAME.to_string(),
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Parse the ingredients blob into structured data. Paprika imports are
        // plain text the user typed (or Ramekin exported), not scraped HTML.
        // Text-recipe drafts record no method.
        let method_used: Option<ExtractionMethod> = match extract_output
            .get("method_used")
            .map(|_| {
                deserialize_required_output_field(&extract_output, "extract_recipe", "method_used")
            })
            .transpose()
        {
            Ok(method) => method,
            Err(e) => {
                return StepResult {
                    step_name: Self::NAME.to_string(),
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };
        let ingredients = parse_raw_ingredients(&raw_recipe, method_used);

        let mut weight_stats = MetricConversionStats::default();
        let mut volume_stats = VolumeConversionStats::default();
        for ingredient in &ingredients {
            derived_grams_with_stats(
                &ingredient.item,
                ingredient.note.as_deref(),
                &ingredient.measurements,
                // The pipeline has no learned names; it measures the catalog.
                &Learned::new(),
                &mut weight_stats,
                &mut volume_stats,
            );
        }

        let output = ParseIngredientsOutput {
            ingredients,
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

/// The ingredients a scrape stores for an extracted recipe: its ingredient
/// lines parsed (Paprika text by its own rules), annotated with footnotes and
/// with amounts normalized. Re-parsing a stored recipe uses this too, so it
/// stores what a fresh scrape would.
pub fn parse_raw_ingredients(
    raw_recipe: &RawRecipe,
    method_used: Option<ExtractionMethod>,
) -> Vec<ParsedIngredient> {
    let mut parsed = if method_used == Some(ExtractionMethod::Paprika) {
        parse_paprika_ingredients(&raw_recipe.ingredients)
    } else {
        parse_ingredients(&raw_recipe.ingredients)
    };

    // Annotate ingredients with footnote text extracted from HTML
    if let Some(ref footnotes) = raw_recipe.footnotes {
        apply_footnotes_to_ingredients(&mut parsed, footnotes);
    }

    parsed
        .into_iter()
        .map(|ing| ing.normalize_amounts())
        .collect()
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
            // "* 1 cup flour": asterisks opening the line are a list bullet,
            // which the raw line keeps, not a footnote marker.
            let is_bullet = start == 0 && bytes.get(i) == Some(&b' ');
            // Valid marker: 1-3 asterisks followed by end-of-string or a boundary char
            if !is_bullet
                && (1..=3).contains(&count)
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

#[cfg(test)]
mod tests {
    #[test]
    fn list_bullets_are_not_footnote_markers() {
        assert_eq!(super::extract_trailing_marker("* 1 cup flour"), "");
        assert_eq!(super::extract_trailing_marker("* 1 tsp salt*"), "*");
        assert_eq!(super::extract_trailing_marker("salt* (to taste)"), "*");
    }

    use serde_json::{json, Value as JsonValue};

    use super::ParseIngredientsStep;
    use crate::pipeline::steps::TestOutputStore;
    use crate::pipeline::{PipelineStep, StepContext};

    #[tokio::test]
    async fn malformed_raw_recipe_reports_deserialization_error() {
        let mut outputs = TestOutputStore::default();
        outputs.outputs.insert(
            "extract_recipe".to_string(),
            json!({ "raw_recipe": { "title": "Soup" } }),
        );
        let ctx = StepContext {
            url: "https://example.test/soup",
            outputs: &outputs,
        };

        let result = ParseIngredientsStep.execute(&ctx).await;

        assert!(!result.success);
        let err = result.error.unwrap();
        assert!(
            err.starts_with("Malformed raw_recipe in extract_recipe output:"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn missing_raw_recipe_reports_missing_field() {
        let mut outputs = TestOutputStore::default();
        outputs
            .outputs
            .insert("extract_recipe".to_string(), json!({}));
        let ctx = StepContext {
            url: "https://example.test/soup",
            outputs: &outputs,
        };

        let result = ParseIngredientsStep.execute(&ctx).await;

        assert!(!result.success);
        assert_eq!(
            result.error.as_deref(),
            Some("Missing raw_recipe in extract_recipe output")
        );
    }

    /// Each parsed ingredient's (item, section) for "Part 1:\n1 cup flour"
    /// extracted with `method`.
    async fn item_sections(method: Option<&str>) -> Vec<(JsonValue, JsonValue)> {
        let mut extract = json!({
            "raw_recipe": {
                "title": "Pie",
                "ingredients": "Part 1:\n1 cup flour",
                "instructions": "",
                "image_urls": [],
            },
        });
        if let Some(method) = method {
            extract["method_used"] = json!(method);
        }
        let mut outputs = TestOutputStore::default();
        outputs
            .outputs
            .insert("extract_recipe".to_string(), extract);
        let ctx = StepContext {
            url: "https://example.test/pie",
            outputs: &outputs,
        };
        let result = ParseIngredientsStep.execute(&ctx).await;
        assert!(result.success, "{:?}", result.error);
        result.output["ingredients"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| (i["item"].clone(), i["section"].clone()))
            .collect()
    }

    #[tokio::test]
    async fn paprika_imports_trust_colon_section_headers() {
        assert_eq!(
            item_sections(Some("paprika")).await,
            [(json!("flour"), json!("Part 1"))]
        );
        let scraped = [
            (json!("Part 1:"), JsonValue::Null),
            (json!("flour"), JsonValue::Null),
        ];
        assert_eq!(item_sections(Some("json_ld")).await, scraped);
        // Text-recipe drafts record no method.
        assert_eq!(item_sections(None).await, scraped);
    }
}
