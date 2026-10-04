//! Estimate weights the ingredient catalog lacks (catalog step 3): grams per
//! unit for a catalog food with no density, or no weight for a counted unit.

use std::collections::HashMap;

use serde::Deserialize;

use crate::ai::prompts::estimate_ingredient_weights::{
    render_estimate_ingredient_weights_prompt, ESTIMATE_INGREDIENT_WEIGHTS_PROMPT_NAME,
};
use crate::ai::{
    complete_json, AiClient, AiError, ChatMessage, ChatRequest, Usage, INGREDIENT_MAX_TOKENS,
};

#[derive(Debug, Deserialize)]
struct Response {
    weights: Vec<RawWeight>,
}

#[derive(Debug, Deserialize)]
struct RawWeight {
    food: String,
    unit: String,
    grams: Option<f64>,
}

/// Plausible grams per cup: a cup of leafy herbs is about 20 g, of honey 340.
const CUP_GRAMS: (f64, f64) = (5.0, 700.0);
/// Plausible grams per any other unit: a saffron thread to a large package.
const UNIT_GRAMS: (f64, f64) = (0.01, 5000.0);

pub struct EstimateIngredientWeightsResult {
    /// Grams per (food, unit); None when the model said there's no typical
    /// weight.
    pub weights: HashMap<(String, String), Option<f64>>,
    pub cached: bool,
    pub usage: Usage,
}

/// Estimate a batch of (food, unit) weights in one call. The answer must cover
/// every pair exactly once with a plausible weight or null; anything else is
/// an invalid response, evicted from the cache so a retry asks again.
/// `fresh` skips any cached answer: a re-ask wants the model's answer now,
/// not the one it gave this prompt before.
pub async fn estimate_ingredient_weights(
    ai_client: &dyn AiClient,
    items: &[(String, String)],
    fresh: bool,
) -> Result<EstimateIngredientWeightsResult, AiError> {
    let request = ChatRequest {
        messages: vec![ChatMessage::user(
            render_estimate_ingredient_weights_prompt(items),
        )],
        json_response: true,
        max_tokens: Some(INGREDIENT_MAX_TOKENS),
        temperature: Some(0.0),
    };
    if fresh {
        ai_client.forget(ESTIMATE_INGREDIENT_WEIGHTS_PROMPT_NAME, &request.messages);
    }
    let (parsed, response): (Response, _) =
        complete_json(ai_client, ESTIMATE_INGREDIENT_WEIGHTS_PROMPT_NAME, &request).await?;
    match validate(items, parsed) {
        Ok(weights) => Ok(EstimateIngredientWeightsResult {
            weights,
            cached: response.cached,
            usage: response.usage,
        }),
        Err(message) => {
            ai_client.forget(ESTIMATE_INGREDIENT_WEIGHTS_PROMPT_NAME, &request.messages);
            Err(AiError::ParseError(message))
        }
    }
}

fn validate(
    items: &[(String, String)],
    parsed: Response,
) -> Result<HashMap<(String, String), Option<f64>>, String> {
    let mut weights = HashMap::new();
    for raw in parsed.weights {
        let key = (raw.food, raw.unit);
        if !items.contains(&key) {
            return Err(format!("answered a pair that wasn't asked: {key:?}"));
        }
        let (min, max) = if key.1 == "cup" {
            CUP_GRAMS
        } else {
            UNIT_GRAMS
        };
        if let Some(grams) = raw.grams {
            if !(grams.is_finite() && (min..=max).contains(&grams)) {
                return Err(format!("{key:?}: implausible weight {grams} g"));
            }
        }
        if weights.insert(key.clone(), raw.grams).is_some() {
            return Err(format!("answered {key:?} twice"));
        }
    }
    if let Some(missing) = items.iter().find(|key| !weights.contains_key(*key)) {
        return Err(format!("no answer for {missing:?}"));
    }
    Ok(weights)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Vec<(String, String)> {
        vec![
            ("capers".into(), "cup".into()),
            ("kale, raw".into(), "bunch".into()),
        ]
    }

    fn response(json: &str) -> Response {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn accepts_complete_plausible_answers() {
        let weights = validate(
            &items(),
            response(
                r#"{"weights": [
            {"food": "capers", "unit": "cup", "grams": 136},
            {"food": "kale, raw", "unit": "bunch", "grams": null}]}"#,
            ),
        )
        .unwrap();
        assert_eq!(weights[&("capers".into(), "cup".into())], Some(136.0));
        assert_eq!(weights[&("kale, raw".into(), "bunch".into())], None);
    }

    #[test]
    fn rejects_implausible_missing_extra_and_duplicate_answers() {
        for json in [
            // A cup of capers can't weigh 2 kg.
            r#"{"weights": [{"food": "capers", "unit": "cup", "grams": 2000}, {"food": "kale, raw", "unit": "bunch", "grams": 200}]}"#,
            r#"{"weights": [{"food": "capers", "unit": "cup", "grams": 0}, {"food": "kale, raw", "unit": "bunch", "grams": 200}]}"#,
            r#"{"weights": [{"food": "capers", "unit": "cup", "grams": 136}]}"#,
            r#"{"weights": [{"food": "capers", "unit": "cup", "grams": 136}, {"food": "capers", "unit": "cup", "grams": 136}, {"food": "kale, raw", "unit": "bunch", "grams": 200}]}"#,
            r#"{"weights": [{"food": "capers", "unit": "jar", "grams": 100}, {"food": "capers", "unit": "cup", "grams": 136}, {"food": "kale, raw", "unit": "bunch", "grams": 200}]}"#,
        ] {
            assert!(validate(&items(), response(json)).is_err(), "{json}");
        }
    }
}
