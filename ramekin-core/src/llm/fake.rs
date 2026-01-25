//! Fake LLM provider for testing.
//!
//! This provider returns deterministic responses based on prompt matching,
//! allowing tests to run without network access or API costs.

use super::{LlmError, LlmProvider};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

/// A fake LLM provider for testing.
///
/// Responses are matched by checking if the prompt contains a registered substring.
/// If no match is found, returns a default response or error.
#[derive(Debug)]
pub struct FakeProvider {
    /// Map of prompt substring -> response
    responses: RwLock<HashMap<String, String>>,
    /// Default response if no match found
    default_response: Option<String>,
}

impl Default for FakeProvider {
    fn default() -> Self {
        Self {
            responses: RwLock::new(HashMap::new()),
            default_response: Some("{}".to_string()),
        }
    }
}

#[allow(dead_code)]
impl FakeProvider {
    /// Create a new FakeProvider with no registered responses.
    pub fn new() -> Self {
        Self {
            responses: RwLock::new(HashMap::new()),
            default_response: None,
        }
    }

    /// Create a FakeProvider that returns a specific response for prompts containing a substring.
    pub fn with_response(prompt_contains: &str, response: &str) -> Self {
        let mut provider = Self::new();
        provider.add_response(prompt_contains, response);
        provider
    }

    /// Add a response for prompts containing a specific substring.
    pub fn add_response(&mut self, prompt_contains: &str, response: &str) {
        self.responses
            .write()
            .unwrap()
            .insert(prompt_contains.to_string(), response.to_string());
    }

    /// Set the default response when no pattern matches.
    pub fn with_default_response(mut self, response: &str) -> Self {
        self.default_response = Some(response.to_string());
        self
    }

    /// Create a FakeProvider with standard responses for enrichment testing.
    pub fn with_enrichment_responses() -> Self {
        let mut provider = Self::new();

        // NormalizeIngredients response
        provider.add_response(
            "normalize",
            r#"[
                {"amount": "1", "unit": "cup", "item": "all-purpose flour", "note": null},
                {"amount": "2", "unit": "tablespoons", "item": "unsalted butter", "note": "softened"}
            ]"#,
        );

        // NormalizeTimes response
        provider.add_response(
            "time",
            r#"{"prep_time": "15 minutes", "cook_time": "30 minutes", "total_time": "45 minutes"}"#,
        );

        // AddNutrition response
        provider.add_response(
            "nutrition",
            "Calories: 250 per serving\nProtein: 8g\nCarbohydrates: 35g\nFat: 10g",
        );

        // ImproveInstructions response
        provider.add_response(
            "instruction",
            "1. Preheat oven to 350°F (175°C).\n2. Mix dry ingredients in a large bowl.\n3. Add wet ingredients and stir until just combined.\n4. Bake for 25-30 minutes until golden brown.",
        );

        provider
    }
}

#[async_trait]
impl LlmProvider for FakeProvider {
    async fn complete(&self, prompt: &str) -> Result<String, LlmError> {
        let responses = self.responses.read().unwrap();

        // Find first matching pattern (case-insensitive)
        let prompt_lower = prompt.to_lowercase();
        for (pattern, response) in responses.iter() {
            if prompt_lower.contains(&pattern.to_lowercase()) {
                return Ok(response.clone());
            }
        }

        // Return default or error
        match &self.default_response {
            Some(response) => Ok(response.clone()),
            None => Err(LlmError::RequestFailed(format!(
                "FakeProvider: No response configured for prompt (first 100 chars): {}",
                &prompt[..prompt.len().min(100)]
            ))),
        }
    }

    fn provider_name(&self) -> &'static str {
        "fake"
    }

    fn model_name(&self) -> &str {
        "fake-model"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fake_provider_matching() {
        let provider = FakeProvider::with_response("hello", "world");
        let result = provider.complete("Say hello to the user").await.unwrap();
        assert_eq!(result, "world");
    }

    #[tokio::test]
    async fn test_fake_provider_case_insensitive() {
        let provider = FakeProvider::with_response("HELLO", "world");
        let result = provider.complete("hello there").await.unwrap();
        assert_eq!(result, "world");
    }

    #[tokio::test]
    async fn test_fake_provider_no_match() {
        let provider = FakeProvider::new();
        let result = provider.complete("random prompt").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fake_provider_default_response() {
        let provider = FakeProvider::new().with_default_response("default");
        let result = provider.complete("random prompt").await.unwrap();
        assert_eq!(result, "default");
    }

    #[tokio::test]
    async fn test_enrichment_responses() {
        let provider = FakeProvider::with_enrichment_responses();

        let result = provider
            .complete("Please normalize these ingredients")
            .await
            .unwrap();
        assert!(result.contains("all-purpose flour"));

        let result = provider.complete("What is the total time?").await.unwrap();
        assert!(result.contains("45 minutes"));
    }
}
