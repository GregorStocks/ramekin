use std::time::Instant;

use async_trait::async_trait;

use ramekin_core::ai::text_extract::extract_recipe_from_text;
use ramekin_core::ai::{AiConfig, CachingAiClient};
use ramekin_core::pipeline::steps::ExtractRecipeStep;
use ramekin_core::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};
use ramekin_core::{ExtractRecipeOutput, ExtractionMethod, RawRecipe};

use super::super::jobs::SOURCE_TEXT_STEP;
use super::helpers::step_failure;

/// `extract_recipe` for a recipe pasted as text, on import and on Re-extract:
/// reads the text with the text extraction model. The model's warnings are
/// kept in the output for the job status page.
pub struct ExtractTextStep;

#[async_trait]
impl PipelineStep for ExtractTextStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: ExtractRecipeStep::NAME,
            description: "Extract the recipe from its pasted text",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();
        let fail = |error: String| step_failure(ExtractRecipeStep::NAME, start, error);
        let Some(text) = ctx
            .outputs
            .get_output(SOURCE_TEXT_STEP)
            .await
            .and_then(|output| output["text"].as_str().map(str::to_string))
        else {
            return fail("source_text output not found".to_string());
        };
        let client = match AiConfig::from_env() {
            Ok(config) => CachingAiClient::new(config.for_extraction()),
            Err(e) => return fail(format!("AI is not configured: {e}")),
        };
        let extracted = match extract_recipe_from_text(&client, &text).await {
            Ok(extracted) => extracted,
            Err(e) => return fail(format!("Text extraction failed: {e}")),
        };
        if let Some(error) = missing_recipe_error(&extracted.raw_recipe, &extracted.warnings) {
            return fail(error);
        }
        let output = ExtractRecipeOutput {
            raw_recipe: extracted.raw_recipe,
            method_used: ExtractionMethod::Text,
            all_attempts: Vec::new(),
        };
        let mut output =
            serde_json::to_value(output).expect("ExtractRecipeOutput serializes to JSON");
        if !extracted.warnings.is_empty() {
            output["warnings"] = serde_json::json!(extracted.warnings);
        }
        StepResult {
            step_name: ExtractRecipeStep::NAME.to_string(),
            success: true,
            output,
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: None,
        }
    }
}

/// Why `raw` is not a recipe worth saving, if it isn't: non-recipe text comes
/// back with these fields empty. The model's warnings say what it saw.
fn missing_recipe_error(raw: &RawRecipe, warnings: &[String]) -> Option<String> {
    let missing: Vec<&str> = [
        ("title", &raw.title),
        ("ingredients", &raw.ingredients),
        ("instructions", &raw.instructions),
    ]
    .into_iter()
    .filter(|(_, value)| value.trim().is_empty())
    .map(|(name, _)| name)
    .collect();
    let is_recipe = !missing.contains(&"title") && missing.len() < 2;
    if is_recipe {
        return None;
    }
    let mut error = format!(
        "No recipe found in the text (missing {}).",
        missing.join(", ")
    );
    for warning in warnings {
        error.push(' ');
        error.push_str(warning);
    }
    Some(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(title: &str, ingredients: &str, instructions: &str) -> RawRecipe {
        serde_json::from_value(serde_json::json!({
            "title": title,
            "ingredients": ingredients,
            "instructions": instructions,
            "image_urls": [],
        }))
        .unwrap()
    }

    #[test]
    fn a_titled_recipe_needs_ingredients_or_instructions() {
        assert_eq!(
            missing_recipe_error(&raw("Soup", "1 cup water", "Boil."), &[]),
            None
        );
        assert_eq!(
            missing_recipe_error(&raw("Soup", "1 cup water", ""), &[]),
            None
        );
        assert_eq!(missing_recipe_error(&raw("Soup", "", "Boil."), &[]), None);
        assert_eq!(
            missing_recipe_error(&raw("Soup", " ", ""), &[]).as_deref(),
            Some("No recipe found in the text (missing ingredients, instructions).")
        );
    }

    #[test]
    fn an_untitled_recipe_is_not_saved_and_says_why() {
        assert_eq!(
            missing_recipe_error(
                &raw("", "1 cup flour", ""),
                &["Check the missing fields.".to_string()]
            )
            .as_deref(),
            Some("No recipe found in the text (missing title, instructions). Check the missing fields.")
        );
    }
}
