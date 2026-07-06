//! Custom enrichment: apply user-specified changes to recipes via AI.

use crate::ai::prompts::custom_enrich::{
    render_custom_enrich_system_prompt, render_custom_enrich_user_prompt, CUSTOM_ENRICH_PROMPT_NAME,
};
use crate::ai::{complete_json, AiClient, AiError, ChatMessage, ChatRequest, ImageData, Usage};

/// Result of custom enrichment.
pub struct CustomEnrichResult<T> {
    /// The modified recipe parsed into the caller's expected shape.
    pub recipe: T,
    pub cached: bool,
    pub usage: Usage,
}

/// Apply a user-specified change to a recipe using AI.
///
/// Takes the recipe as a JSON string and the user's instruction describing
/// what change to make. Returns the complete modified recipe parsed into the
/// caller's expected shape.
pub async fn custom_enrich<T: serde::de::DeserializeOwned>(
    ai_client: &dyn AiClient,
    recipe_json: &str,
    instruction: &str,
    images: Vec<ImageData>,
) -> Result<CustomEnrichResult<T>, AiError> {
    let system_prompt = render_custom_enrich_system_prompt();
    let user_prompt = render_custom_enrich_user_prompt(recipe_json, instruction);
    let user_message = if images.is_empty() {
        ChatMessage::user(user_prompt)
    } else {
        ChatMessage::user_with_images(user_prompt, images)
    };

    let request = ChatRequest {
        messages: vec![ChatMessage::system(system_prompt), user_message],
        json_response: true,
        max_tokens: Some(4096),
        temperature: Some(0.7),
    };

    let (recipe, response): (T, _) =
        complete_json(ai_client, CUSTOM_ENRICH_PROMPT_NAME, &request).await?;

    Ok(CustomEnrichResult {
        recipe,
        cached: response.cached,
        usage: response.usage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{ChatResponse, Usage};
    use async_trait::async_trait;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct FakeClient {
        responses: Mutex<VecDeque<ChatResponse>>,
        forgotten: Mutex<Vec<String>>,
    }

    impl FakeClient {
        fn new(responses: Vec<ChatResponse>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
                forgotten: Mutex::new(vec![]),
            }
        }
    }

    #[async_trait]
    impl AiClient for FakeClient {
        async fn complete(
            &self,
            _prompt_name: &str,
            _request: &ChatRequest,
        ) -> Result<ChatResponse, AiError> {
            Ok(self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected extra complete() call"))
        }

        fn forget(&self, prompt_name: &str, _messages: &[ChatMessage]) {
            self.forgotten.lock().unwrap().push(prompt_name.to_string());
        }
    }

    #[derive(serde::Deserialize)]
    struct ExpectedRecipe {
        title: String,
    }

    fn response(content: &str, cached: bool) -> ChatResponse {
        ChatResponse {
            content: content.to_string(),
            usage: Usage::default(),
            cached,
        }
    }

    #[tokio::test]
    async fn custom_enrich_evicts_fresh_wrong_shape_response() {
        let client = FakeClient::new(vec![response(r#"{"error": "try again"}"#, false)]);

        let result = custom_enrich::<ExpectedRecipe>(&client, "{}", "fix it", vec![]).await;

        assert!(matches!(result, Err(AiError::ParseError(_))));
        assert_eq!(
            *client.forgotten.lock().unwrap(),
            vec![CUSTOM_ENRICH_PROMPT_NAME]
        );
    }

    #[tokio::test]
    async fn custom_enrich_retries_cached_wrong_shape_response() {
        let client = FakeClient::new(vec![
            response(r#"{"error": "try again"}"#, true),
            response(r#"{"title": "Fresh title"}"#, false),
        ]);

        let result = custom_enrich::<ExpectedRecipe>(&client, "{}", "fix it", vec![])
            .await
            .unwrap();

        assert_eq!(result.recipe.title, "Fresh title");
        assert!(!result.cached);
        assert_eq!(
            *client.forgotten.lock().unwrap(),
            vec![CUSTOM_ENRICH_PROMPT_NAME]
        );
    }
}
