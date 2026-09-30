//! Resolve ingredient names the catalog doesn't know (catalog step 3).

use std::collections::HashMap;

use serde::Deserialize;

use crate::ai::prompts::resolve_ingredient_names::{
    render_resolve_ingredient_names_prompt, RESOLVE_INGREDIENT_NAMES_PROMPT_NAME,
};
use crate::ai::{complete_json, AiClient, AiError, ChatMessage, ChatRequest, Usage};

#[derive(Debug, Deserialize)]
struct Response {
    resolutions: Vec<RawResolution>,
}

#[derive(Debug, Deserialize)]
struct RawResolution {
    name: String,
    answer: String,
    key: Option<String>,
}

/// The answer for one name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameResolution {
    /// A catalog key from that name's candidates.
    Entry(String),
    NotFood,
    Unknown,
}

pub struct ResolveIngredientNamesResult {
    pub resolutions: HashMap<String, NameResolution>,
    pub cached: bool,
    pub usage: Usage,
}

/// Resolve a batch of names, each with its candidate catalog keys, in one
/// call. The answer must cover every name exactly once and may only choose a
/// listed candidate; anything else is an invalid response, evicted from the
/// cache so a retry asks again.
pub async fn resolve_ingredient_names(
    ai_client: &dyn AiClient,
    names: &[(String, Vec<String>)],
) -> Result<ResolveIngredientNamesResult, AiError> {
    let request = ChatRequest {
        messages: vec![ChatMessage::user(render_resolve_ingredient_names_prompt(
            names,
        ))],
        json_response: true,
        max_tokens: Some(4096),
        temperature: Some(0.0),
    };
    let (parsed, response): (Response, _) =
        complete_json(ai_client, RESOLVE_INGREDIENT_NAMES_PROMPT_NAME, &request).await?;
    match validate(names, parsed) {
        Ok(resolutions) => Ok(ResolveIngredientNamesResult {
            resolutions,
            cached: response.cached,
            usage: response.usage,
        }),
        Err(message) => {
            ai_client.forget(RESOLVE_INGREDIENT_NAMES_PROMPT_NAME, &request.messages);
            Err(AiError::ParseError(message))
        }
    }
}

fn validate(
    names: &[(String, Vec<String>)],
    parsed: Response,
) -> Result<HashMap<String, NameResolution>, String> {
    let candidates: HashMap<&str, &Vec<String>> = names
        .iter()
        .map(|(name, keys)| (name.as_str(), keys))
        .collect();
    let mut resolutions = HashMap::new();
    for raw in parsed.resolutions {
        let Some(keys) = candidates.get(raw.name.as_str()) else {
            return Err(format!("answered a name that wasn't asked: {:?}", raw.name));
        };
        let resolution = match (raw.answer.as_str(), raw.key) {
            ("entry", Some(key)) if keys.contains(&key) => NameResolution::Entry(key),
            ("entry", key) => {
                return Err(format!(
                    "{:?}: key {key:?} is not one of its candidates",
                    raw.name
                ))
            }
            ("not_food", _) => NameResolution::NotFood,
            ("unknown", _) => NameResolution::Unknown,
            (other, _) => return Err(format!("{:?}: unknown answer {other:?}", raw.name)),
        };
        if resolutions.insert(raw.name.clone(), resolution).is_some() {
            return Err(format!("answered {:?} twice", raw.name));
        }
    }
    if let Some((missing, _)) = names
        .iter()
        .find(|(name, _)| !resolutions.contains_key(name))
    {
        return Err(format!("no answer for {missing:?}"));
    }
    Ok(resolutions)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> Vec<(String, Vec<String>)> {
        vec![
            ("moon sugar".into(), vec!["granulated sugar".into()]),
            ("platter".into(), vec![]),
        ]
    }

    fn response(json: &str) -> Response {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn accepts_complete_answers_within_candidates() {
        let resolved = validate(
            &names(),
            response(
                r#"{"resolutions": [
            {"name": "moon sugar", "answer": "entry", "key": "granulated sugar"},
            {"name": "platter", "answer": "not_food"}]}"#,
            ),
        )
        .unwrap();
        assert_eq!(
            resolved["moon sugar"],
            NameResolution::Entry("granulated sugar".into())
        );
        assert_eq!(resolved["platter"], NameResolution::NotFood);
    }

    #[test]
    fn rejects_invented_keys_missing_names_and_duplicates() {
        for json in [
            r#"{"resolutions": [{"name": "moon sugar", "answer": "entry", "key": "honey"}, {"name": "platter", "answer": "unknown"}]}"#,
            r#"{"resolutions": [{"name": "moon sugar", "answer": "unknown"}]}"#,
            r#"{"resolutions": [{"name": "moon sugar", "answer": "unknown"}, {"name": "moon sugar", "answer": "unknown"}, {"name": "platter", "answer": "unknown"}]}"#,
            r#"{"resolutions": [{"name": "stardust", "answer": "unknown"}, {"name": "moon sugar", "answer": "unknown"}, {"name": "platter", "answer": "unknown"}]}"#,
            r#"{"resolutions": [{"name": "moon sugar", "answer": "maybe"}, {"name": "platter", "answer": "unknown"}]}"#,
        ] {
            assert!(validate(&names(), response(json)).is_err(), "{json}");
        }
    }
}
