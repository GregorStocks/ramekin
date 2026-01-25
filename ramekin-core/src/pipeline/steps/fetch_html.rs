//! FetchHtml step - fetches HTML from a URL using an injected HTTP client.

use std::time::Instant;

use async_trait::async_trait;
use serde_json::json;

use crate::http::HttpClient;
use crate::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};

/// Step that fetches HTML from a URL.
///
/// The HTTP client is injected at construction time, allowing CLI to use
/// CachingClient (with disk cache) and server to use its own client.
pub struct FetchHtmlStep<C: HttpClient> {
    client: C,
}

impl<C: HttpClient> FetchHtmlStep<C> {
    /// Step name constant.
    pub const NAME: &'static str = "fetch_html";

    /// Create a new FetchHtmlStep with the given HTTP client.
    pub fn new(client: C) -> Self {
        Self { client }
    }
}

#[async_trait]
impl<C: HttpClient + Send + Sync> PipelineStep for FetchHtmlStep<C> {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Fetch HTML from URL",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        match self.client.fetch_html(ctx.url).await {
            Ok(html) => StepResult {
                success: true,
                output: json!({ "html": html }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: Some("extract_recipe".to_string()),
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
