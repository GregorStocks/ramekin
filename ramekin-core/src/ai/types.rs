//! AI request and response types.

use serde::{Deserialize, Serialize};

/// Role in a chat conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    /// A tool's result, answering the assistant's tool call `tool_call_id`.
    Tool,
}

/// A tool the model may call: its name, what it does, and a JSON Schema for
/// its arguments.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// A call the model asked for: the tool's name and its arguments as JSON
/// text (the model's, so possibly malformed).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Image data for vision API requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    /// Base64-encoded image bytes.
    pub base64: String,
    /// MIME type (e.g., "image/jpeg", "image/png").
    pub content_type: String,
}

/// A message in a chat conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    /// Optional images for vision requests. Empty by default.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImageData>,
    /// The tools an assistant message called.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// The call a tool message answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            images: vec![],
            tool_calls: vec![],
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            images: vec![],
            tool_calls: vec![],
            tool_call_id: None,
        }
    }

    pub fn user_with_images(content: impl Into<String>, images: Vec<ImageData>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            images,
            tool_calls: vec![],
            tool_call_id: None,
        }
    }

    /// An assistant turn that called tools, to replay in the history.
    pub fn assistant_tool_calls(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            images: vec![],
            tool_calls,
            tool_call_id: None,
        }
    }

    /// A tool's result for the call `tool_call_id`.
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            images: vec![],
            tool_calls: vec![],
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            images: vec![],
            tool_calls: vec![],
            tool_call_id: None,
        }
    }
}

/// Output-token budget for prompts that expect a short JSON answer.
///
/// Generous on purpose: reasoning models spend output tokens on hidden
/// thinking before the answer, and a too-small cap truncates the answer
/// itself (finish_reason=length).
pub const SHORT_JSON_ANSWER_MAX_TOKENS: u32 = 4096;

/// Request for a chat completion.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// If true, request JSON response format.
    #[serde(skip)]
    pub json_response: bool,
    /// Tools the model may call instead of answering.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolSpec>,
}

/// Token usage information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Response from a chat completion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChatResponse {
    /// The generated content.
    pub content: String,
    /// Token usage statistics.
    pub usage: Usage,
    /// Whether this response came from cache.
    #[serde(default)]
    pub cached: bool,
    /// The tools the model called, if it called any instead of answering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}
