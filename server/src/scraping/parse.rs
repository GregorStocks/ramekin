use crate::models::Ingredient;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("No Recipe found in JSON-LD")]
    NoRecipe,

    #[error("Invalid JSON-LD: {0}")]
    InvalidJson(String),

    #[error("Missing required field: {0}")]
    MissingField(String),
}

/// Parsed recipe data extracted from JSON-LD
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedRecipe {
    pub title: String,
    pub description: Option<String>,
    pub ingredients: Vec<Ingredient>,
    pub instructions: String,
    pub source_name: Option<String>,
}

/// Parse a Recipe from HTML containing JSON-LD structured data.
pub fn parse_recipe_from_html(html: &str, source_url: &str) -> Result<ParsedRecipe, ParseError> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("script[type='application/ld+json']").expect("Invalid selector");

    for element in document.select(&selector) {
        let json_text = element.inner_html();

        // Try to parse as JSON
        let json: serde_json::Value = match serde_json::from_str(&json_text) {
            Ok(v) => v,
            Err(_) => continue, // Try next script tag
        };

        // Look for Recipe type
        if let Some(recipe) = find_recipe_in_json(&json) {
            return extract_recipe_data(recipe, source_url);
        }
    }

    Err(ParseError::NoRecipe)
}

/// Recursively search for a Recipe object in JSON-LD.
/// Handles @graph arrays and nested structures.
fn find_recipe_in_json(json: &serde_json::Value) -> Option<&serde_json::Value> {
    match json {
        serde_json::Value::Object(obj) => {
            // Check if this object is a Recipe
            if let Some(type_val) = obj.get("@type") {
                let is_recipe = match type_val {
                    serde_json::Value::String(s) => s == "Recipe",
                    serde_json::Value::Array(arr) => arr.iter().any(|v| v == "Recipe"),
                    _ => false,
                };
                if is_recipe {
                    return Some(json);
                }
            }

            // Check @graph for array of items
            if let Some(graph) = obj.get("@graph") {
                if let Some(recipe) = find_recipe_in_json(graph) {
                    return Some(recipe);
                }
            }

            // Recursively search other fields
            for (_, value) in obj {
                if let Some(recipe) = find_recipe_in_json(value) {
                    return Some(recipe);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                if let Some(recipe) = find_recipe_in_json(item) {
                    return Some(recipe);
                }
            }
        }
        _ => {}
    }
    None
}

/// Extract recipe data from a JSON-LD Recipe object.
fn extract_recipe_data(
    recipe: &serde_json::Value,
    source_url: &str,
) -> Result<ParsedRecipe, ParseError> {
    let title = recipe
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ParseError::MissingField("name".to_string()))?
        .to_string();

    let description = recipe
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let ingredients = extract_ingredients(recipe)?;
    let instructions = extract_instructions(recipe)?;
    let source_name = extract_source_name(source_url);

    Ok(ParsedRecipe {
        title,
        description,
        ingredients,
        instructions,
        source_name,
    })
}

/// Extract ingredients from recipeIngredient field.
fn extract_ingredients(recipe: &serde_json::Value) -> Result<Vec<Ingredient>, ParseError> {
    let ingredients_raw = recipe
        .get("recipeIngredient")
        .ok_or_else(|| ParseError::MissingField("recipeIngredient".to_string()))?;

    let ingredients_array = ingredients_raw
        .as_array()
        .ok_or_else(|| ParseError::InvalidJson("recipeIngredient is not an array".to_string()))?;

    let ingredients: Vec<Ingredient> = ingredients_array
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| Ingredient {
            item: s.trim().to_string(),
            amount: None,
            unit: None,
            note: None,
        })
        .collect();

    if ingredients.is_empty() {
        return Err(ParseError::MissingField(
            "recipeIngredient (empty)".to_string(),
        ));
    }

    Ok(ingredients)
}

/// Extract instructions from recipeInstructions field.
/// Handles both string and array formats.
fn extract_instructions(recipe: &serde_json::Value) -> Result<String, ParseError> {
    let instructions_raw = recipe
        .get("recipeInstructions")
        .ok_or_else(|| ParseError::MissingField("recipeInstructions".to_string()))?;

    match instructions_raw {
        serde_json::Value::String(s) => Ok(s.trim().to_string()),
        serde_json::Value::Array(arr) => {
            let steps: Vec<String> = arr
                .iter()
                .filter_map(|item| {
                    // Handle HowToStep objects
                    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                        return Some(text.trim().to_string());
                    }
                    // Handle plain strings
                    if let Some(s) = item.as_str() {
                        return Some(s.trim().to_string());
                    }
                    // Handle HowToSection with itemListElement
                    if let Some(items) = item.get("itemListElement").and_then(|v| v.as_array()) {
                        let section_steps: Vec<String> = items
                            .iter()
                            .filter_map(|step| step.get("text").and_then(|v| v.as_str()))
                            .map(|s| s.trim().to_string())
                            .collect();
                        if !section_steps.is_empty() {
                            return Some(section_steps.join("\n"));
                        }
                    }
                    None
                })
                .collect();

            if steps.is_empty() {
                return Err(ParseError::MissingField(
                    "recipeInstructions (empty)".to_string(),
                ));
            }

            Ok(steps.join("\n\n"))
        }
        _ => Err(ParseError::InvalidJson(
            "recipeInstructions is not a string or array".to_string(),
        )),
    }
}

/// Extract a friendly source name from a URL.
fn extract_source_name(url: &str) -> Option<String> {
    reqwest::Url::parse(url).ok().and_then(|parsed| {
        parsed.host_str().map(|host| {
            // Remove www. prefix and common TLDs for cleaner names
            let name = host.strip_prefix("www.").unwrap_or(host);
            // Capitalize first letter
            let mut chars = name.chars();
            match chars.next() {
                None => name.to_string(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_source_name() {
        assert_eq!(
            extract_source_name("https://www.allrecipes.com/recipe/123"),
            Some("Allrecipes.com".to_string())
        );
        assert_eq!(
            extract_source_name("https://cooking.nytimes.com/recipes/456"),
            Some("Cooking.nytimes.com".to_string())
        );
    }
}
