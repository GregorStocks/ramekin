//! AI client module for LLM integration via OpenRouter.
//!
//! This module provides:
//! - `AiClient` trait for abstracting AI providers
//! - `CachingAiClient` implementation with disk-based caching
//! - Configuration via environment variables
//! - Prompt templates for various AI tasks
//!
//! # Configuration
//!
//! Set these environment variables:
//!
//! - `OPENROUTER_API_KEY` (required): Your OpenRouter API key
//! - `RAMEKIN_AI_MODEL` (optional): Model name, e.g., "openai/gpt-4o-mini"
//! - `RAMEKIN_AI_BASE_URL` (optional): API base URL
//! - `RAMEKIN_AI_CACHE_DIR` (optional): Cache directory path
//! - `RAMEKIN_AI_OFFLINE` (optional): Set to "true" to use cache only
//! - `RAMEKIN_AI_RATE_LIMIT_MS` (optional): Delay between requests in ms
//!
//! # Example
//!
//! ```ignore
//! use ramekin_core::ai::{AiClient, CachingAiClient, ChatMessage, ChatRequest};
//!
//! let client = CachingAiClient::from_env()?;
//!
//! let request = ChatRequest {
//!     messages: vec![ChatMessage::user("Hello!")],
//!     ..Default::default()
//! };
//!
//! let response = client.complete("test", "v1", request).await?;
//! println!("Response: {}", response.content);
//! ```

mod auto_tag;
mod cache;
mod client;
mod config;
pub mod prompts;
mod types;

pub use auto_tag::{suggest_tags, AutoTagResult};
pub use cache::{AiCache, CacheKey, CacheStats, CachedAiResponse};
pub use client::{AiClient, AiError, CachingAiClient};
pub use config::{AiConfig, ConfigError};
pub use types::{ChatMessage, ChatRequest, ChatResponse, Role, Usage};
