//! Consolidated end-of-pipeline recipe view.
//!
//! Assembles the final state of a scraped recipe by combining the outputs of
//! `extract_recipe`, `parse_ingredients`, `enrich_auto_tag`, and `apply_auto_tags`.
//! Used by the CLI snapshot writer and (eventually) the server scrape-status view.

use serde::{Deserialize, Serialize};

use crate::ingredient_parser::ParsedIngredient;
use crate::types::RawRecipe;

/// Consolidated end-of-pipeline recipe state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FinalRecipe {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servings: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prep_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cook_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_time: Option<String>,
    pub instructions: String,
    pub image_urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    pub ingredients: Vec<ParsedIngredient>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied_tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_tags: Option<Vec<String>>,
}

/// Build a `FinalRecipe` from the outputs of the relevant pipeline steps.
pub fn build_final_recipe(
    raw_recipe: &RawRecipe,
    parsed_ingredients: Option<&[ParsedIngredient]>,
    suggested_tags: Option<&[String]>,
    applied_tags: Option<&[String]>,
) -> FinalRecipe {
    // Match the server's SaveRecipeStep behaviour: if the parse_ingredients
    // step produced output (even an empty vec), use it; only fall back to
    // line-splitting raw text when the step was absent entirely. Collapsing
    // Some([]) into the fallback would invent ingredients that never existed
    // in the real pipeline output and corrupt snapshot diffs.
    let ingredients = match parsed_ingredients {
        Some(parsed) => parsed.to_vec(),
        None => raw_recipe
            .ingredients
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| ParsedIngredient {
                item: line.trim().to_string(),
                measurements: Vec::new(),
                note: None,
                raw: None,
                section: None,
            })
            .collect(),
    };
    FinalRecipe {
        title: raw_recipe.title.clone(),
        description: raw_recipe.description.clone(),
        servings: raw_recipe.servings.clone(),
        prep_time: raw_recipe.prep_time.clone(),
        cook_time: raw_recipe.cook_time.clone(),
        total_time: raw_recipe.total_time.clone(),
        instructions: raw_recipe.instructions.clone(),
        image_urls: raw_recipe.image_urls.clone(),
        source_url: raw_recipe.source_url.clone(),
        source_name: raw_recipe.source_name.clone(),
        ingredients,
        applied_tags: applied_tags.map(<[String]>::to_vec),
        suggested_tags: suggested_tags.map(<[String]>::to_vec),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_recipe_fixture() -> RawRecipe {
        RawRecipe {
            title: "Test Recipe".to_string(),
            description: Some("A test".to_string()),
            ingredients: "1 cup flour\n2 eggs".to_string(),
            instructions: "Mix and bake.".to_string(),
            image_urls: vec!["https://example.com/img.jpg".to_string()],
            source_url: Some("https://example.com/recipe".to_string()),
            source_name: Some("example.com".to_string()),
            servings: Some("4".to_string()),
            prep_time: Some("10 minutes".to_string()),
            cook_time: Some("20 minutes".to_string()),
            total_time: Some("30 minutes".to_string()),
            rating: None,
            difficulty: None,
            nutritional_info: None,
            notes: None,
            categories: None,
            footnotes: None,
        }
    }

    #[test]
    fn copies_raw_recipe_fields() {
        let raw = raw_recipe_fixture();
        let fr = build_final_recipe(&raw, None, None, None);
        assert_eq!(fr.title, "Test Recipe");
        assert_eq!(fr.description.as_deref(), Some("A test"));
        assert_eq!(fr.instructions, "Mix and bake.");
        assert_eq!(
            fr.image_urls,
            vec!["https://example.com/img.jpg".to_string()]
        );
        assert_eq!(fr.source_name.as_deref(), Some("example.com"));
        assert_eq!(fr.servings.as_deref(), Some("4"));
        assert_eq!(fr.total_time.as_deref(), Some("30 minutes"));
    }

    #[test]
    fn uses_parsed_ingredients_when_present() {
        let raw = raw_recipe_fixture();
        let parsed = vec![ParsedIngredient {
            item: "flour".to_string(),
            measurements: vec![crate::ingredient_parser::Measurement {
                amount: Some("1".to_string()),
                unit: Some("cup".to_string()),
            }],
            note: None,
            raw: Some("1 cup flour".to_string()),
            section: None,
        }];
        let fr = build_final_recipe(&raw, Some(&parsed), None, None);
        assert_eq!(fr.ingredients.len(), 1);
        assert_eq!(fr.ingredients[0].item, "flour");
    }

    #[test]
    fn falls_back_to_line_split_when_parsed_absent() {
        let raw = raw_recipe_fixture();
        let fr = build_final_recipe(&raw, None, None, None);
        assert_eq!(fr.ingredients.len(), 2);
        assert_eq!(fr.ingredients[0].item, "1 cup flour");
        assert_eq!(fr.ingredients[0].measurements, Vec::new());
        assert_eq!(fr.ingredients[1].item, "2 eggs");
    }

    #[test]
    fn preserves_empty_ingredients_when_parse_step_produced_empty() {
        // When parse_ingredients emits an empty vec we trust it — the server's
        // SaveRecipeStep does the same. Falling back to line-splitting raw
        // text here would invent ingredients that aren't really in the recipe.
        let raw = raw_recipe_fixture();
        let empty: Vec<ParsedIngredient> = Vec::new();
        let fr = build_final_recipe(&raw, Some(&empty), None, None);
        assert!(fr.ingredients.is_empty());
    }

    #[test]
    fn line_split_skips_blank_lines() {
        let mut raw = raw_recipe_fixture();
        raw.ingredients = "1 cup flour\n\n   \n2 eggs".to_string();
        let fr = build_final_recipe(&raw, None, None, None);
        assert_eq!(fr.ingredients.len(), 2);
        assert_eq!(fr.ingredients[0].item, "1 cup flour");
        assert_eq!(fr.ingredients[1].item, "2 eggs");
    }

    #[test]
    fn passes_suggested_tags_through() {
        let raw = raw_recipe_fixture();
        let suggested = vec!["dinner".to_string(), "mexican".to_string()];
        let fr = build_final_recipe(&raw, None, Some(&suggested), None);
        assert_eq!(fr.suggested_tags.as_deref(), Some(&suggested[..]));
        assert!(fr.applied_tags.is_none());
    }

    #[test]
    fn passes_applied_tags_through() {
        let raw = raw_recipe_fixture();
        let applied = vec!["vegetarian".to_string()];
        let fr = build_final_recipe(&raw, None, None, Some(&applied));
        assert_eq!(fr.applied_tags.as_deref(), Some(&applied[..]));
        assert!(fr.suggested_tags.is_none());
    }

    #[test]
    fn passes_both_tag_fields_through() {
        let raw = raw_recipe_fixture();
        let suggested = vec!["dinner".to_string()];
        let applied = vec!["dinner".to_string(), "mexican".to_string()];
        let fr = build_final_recipe(&raw, None, Some(&suggested), Some(&applied));
        assert_eq!(fr.suggested_tags.as_deref(), Some(&suggested[..]));
        assert_eq!(fr.applied_tags.as_deref(), Some(&applied[..]));
    }

    #[test]
    fn serializes_without_null_fields_for_missing_optionals() {
        let raw = raw_recipe_fixture();
        let fr = build_final_recipe(&raw, None, None, None);
        let json = serde_json::to_string(&fr).unwrap();
        assert!(
            !json.contains("\"applied_tags\""),
            "unexpected applied_tags in {json}"
        );
        assert!(
            !json.contains("\"suggested_tags\""),
            "unexpected suggested_tags in {json}"
        );
    }

    #[test]
    fn round_trips_through_serde() {
        let raw = raw_recipe_fixture();
        let suggested = vec!["dinner".to_string()];
        let applied = vec!["vegetarian".to_string()];
        let fr = build_final_recipe(&raw, None, Some(&suggested), Some(&applied));
        let json = serde_json::to_string_pretty(&fr).unwrap();
        let decoded: FinalRecipe = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, fr);
    }
}
