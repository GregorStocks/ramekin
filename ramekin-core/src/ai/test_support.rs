//! Test doubles for the AI client.

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::Value;

use crate::ai::{AiClient, AiError, ChatMessage, ChatRequest, ChatResponse, ToolCall, Usage};

/// Answers each turn with the next scripted tool calls (none once the script
/// runs out), and records the requests it saw. Every turn reports 15 tokens.
pub struct ScriptedClient {
    turns: Mutex<VecDeque<Vec<ToolCall>>>,
    pub requests: Mutex<Vec<ChatRequest>>,
}

impl ScriptedClient {
    pub fn new(turns: Vec<Vec<(&str, Value)>>) -> Self {
        let turns = turns
            .into_iter()
            .enumerate()
            .map(|(turn, calls)| {
                calls
                    .into_iter()
                    .enumerate()
                    .map(|(i, (name, args))| ToolCall {
                        id: format!("call-{turn}-{i}"),
                        name: name.to_string(),
                        arguments: args.to_string(),
                    })
                    .collect()
            })
            .collect();
        Self {
            turns: Mutex::new(turns),
            requests: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl AiClient for ScriptedClient {
    async fn complete(
        &self,
        _prompt_name: &str,
        request: &ChatRequest,
    ) -> Result<ChatResponse, AiError> {
        self.requests.lock().unwrap().push(request.clone());
        let tool_calls = self.turns.lock().unwrap().pop_front().unwrap_or_default();
        Ok(ChatResponse {
            content: String::new(),
            usage: Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            },
            cached: false,
            tool_calls,
        })
    }

    fn forget(&self, _prompt_name: &str, _messages: &[ChatMessage]) {}
}
