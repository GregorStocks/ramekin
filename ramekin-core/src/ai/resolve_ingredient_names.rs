//! Resolve ingredient names the catalog doesn't know (catalog step 3).

use std::collections::HashMap;

use serde::Deserialize;

use crate::ai::prompts::resolve_ingredient_names::{
    render_resolve_ingredient_names_prompt, NameQuery, RESOLVE_INGREDIENT_NAMES_PROMPT_NAME,
};
use crate::ai::{
    complete_json, AiClient, AiError, ChatMessage, ChatRequest, Usage, INGREDIENT_MAX_TOKENS,
};
use crate::catalog::EstimatedFood;

#[derive(Debug, Deserialize)]
struct Response {
    resolutions: Vec<RawResolution>,
}

#[derive(Debug, Deserialize)]
struct RawResolution {
    name: String,
    answer: String,
    key: Option<String>,
    kcal_per_100g: Option<f64>,
    grams_per_cup: Option<f64>,
    grams_per_piece: Option<f64>,
}

/// The answer for one name.
#[derive(Debug, Clone, PartialEq)]
pub enum NameResolution {
    /// A catalog key from that name's candidates.
    Entry(String),
    /// A real food no candidate matches, with the model's numbers.
    Estimate(EstimatedFood),
    NotFood,
    Unknown,
}

/// Plausible calories per 100 g: water to pure fat.
const KCAL_PER_100G: (f64, f64) = (0.0, 900.0);
/// Plausible grams per cup and per piece, as for estimated weights.
const CUP_GRAMS: (f64, f64) = (5.0, 700.0);
const PIECE_GRAMS: (f64, f64) = (0.01, 5000.0);

fn within(value: f64, (min, max): (f64, f64)) -> bool {
    value.is_finite() && (min..=max).contains(&value)
}

pub struct ResolveIngredientNamesResult {
    pub resolutions: HashMap<String, NameResolution>,
    pub cached: bool,
    pub usage: Usage,
}

/// Resolve a batch of names, each with its candidate catalog keys, in one
/// call. The answer must cover every name exactly once and may only choose a
/// listed candidate; anything else is an invalid response, evicted from the
/// cache so a retry asks again. `fresh` skips a cached reply: re-asking a name
/// must reach the model, not return the answer being replaced.
pub async fn resolve_ingredient_names(
    ai_client: &dyn AiClient,
    names: &[NameQuery],
    fresh: bool,
) -> Result<ResolveIngredientNamesResult, AiError> {
    let request = ChatRequest {
        messages: vec![ChatMessage::user(render_resolve_ingredient_names_prompt(
            names,
        ))],
        json_response: true,
        fresh,
        max_tokens: Some(INGREDIENT_MAX_TOKENS),
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
    names: &[NameQuery],
    parsed: Response,
) -> Result<HashMap<String, NameResolution>, String> {
    let queries: HashMap<&str, &NameQuery> = names
        .iter()
        .map(|query| (query.name.as_str(), query))
        .collect();
    let mut resolutions = HashMap::new();
    for raw in parsed.resolutions {
        let Some(query) = queries.get(raw.name.as_str()) else {
            return Err(format!("answered a name that wasn't asked: {:?}", raw.name));
        };
        let resolution = match (raw.answer.as_str(), raw.key) {
            ("entry", Some(key)) if query.candidates.contains(&key) => NameResolution::Entry(key),
            ("entry", key) => {
                return Err(format!(
                    "{:?}: key {key:?} is not one of its candidates",
                    raw.name
                ))
            }
            // An ambiguous name is a catalog food with several candidates:
            // the model picks one or says it can't. It never invents numbers
            // for it, and never calls it not food, which would drop it from
            // every estimate.
            ("estimate" | "not_food", _) if query.ambiguous => {
                return Err(format!(
                    "{:?}: answered {:?} for an ambiguous name",
                    raw.name, raw.answer
                ))
            }
            ("estimate", _) => {
                let kcal = raw.kcal_per_100g.filter(|v| within(*v, KCAL_PER_100G));
                let cup_ok = raw.grams_per_cup.is_none_or(|v| within(v, CUP_GRAMS));
                let piece_ok = raw.grams_per_piece.is_none_or(|v| within(v, PIECE_GRAMS));
                match kcal {
                    Some(kcal_per_100g) if cup_ok && piece_ok => {
                        NameResolution::Estimate(EstimatedFood {
                            kcal_per_100g,
                            grams_per_cup: raw.grams_per_cup,
                            grams_per_piece: raw.grams_per_piece,
                        })
                    }
                    _ => {
                        return Err(format!(
                            "{:?}: implausible estimate {:?} kcal/100 g, {:?} g/cup, {:?} g/piece",
                            raw.name, raw.kcal_per_100g, raw.grams_per_cup, raw.grams_per_piece
                        ))
                    }
                }
            }
            ("not_food", _) => NameResolution::NotFood,
            ("unknown", _) => NameResolution::Unknown,
            (other, _) => return Err(format!("{:?}: unknown answer {other:?}", raw.name)),
        };
        if resolutions.insert(raw.name.clone(), resolution).is_some() {
            return Err(format!("answered {:?} twice", raw.name));
        }
    }
    if let Some(missing) = names
        .iter()
        .find(|query| !resolutions.contains_key(&query.name))
    {
        return Err(format!("no answer for {:?}", missing.name));
    }
    Ok(resolutions)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> Vec<NameQuery> {
        vec![
            NameQuery {
                name: "moon sugar".into(),
                candidates: vec!["granulated sugar".into()],
                ambiguous: false,
            },
            NameQuery {
                name: "platter".into(),
                candidates: vec![],
                ambiguous: false,
            },
            NameQuery {
                name: "cheese".into(),
                candidates: vec!["cheddar cheese".into()],
                ambiguous: true,
            },
        ]
    }

    fn response(json: &str) -> Response {
        serde_json::from_str(json).unwrap()
    }

    const CHEESE: &str = r#"{"name": "cheese", "answer": "entry", "key": "cheddar cheese"}"#;

    fn with_cheese(rest: &str) -> String {
        format!(r#"{{"resolutions": [{rest}, {CHEESE}]}}"#)
    }

    #[test]
    fn accepts_complete_answers_within_candidates() {
        let resolved = validate(
            &names(),
            response(&with_cheese(
                r#"{"name": "moon sugar", "answer": "entry", "key": "granulated sugar"},
            {"name": "platter", "answer": "not_food"}"#,
            )),
        )
        .unwrap();
        assert_eq!(
            resolved["moon sugar"],
            NameResolution::Entry("granulated sugar".into())
        );
        assert_eq!(resolved["platter"], NameResolution::NotFood);
        assert_eq!(
            resolved["cheese"],
            NameResolution::Entry("cheddar cheese".into())
        );
    }

    #[test]
    fn accepts_plausible_estimates_for_unmatched_foods() {
        let resolved = validate(
            &names(),
            response(&with_cheese(
                r#"{"name": "moon sugar", "answer": "estimate", "kcal_per_100g": 380, "grams_per_cup": 200, "grams_per_piece": null},
            {"name": "platter", "answer": "unknown"}"#,
            )),
        )
        .unwrap();
        assert_eq!(
            resolved["moon sugar"],
            NameResolution::Estimate(EstimatedFood {
                kcal_per_100g: 380.0,
                grams_per_cup: Some(200.0),
                grams_per_piece: None,
            })
        );
    }

    #[test]
    fn rejects_invented_keys_bad_estimates_missing_names_and_duplicates() {
        for rest in [
            r#"{"name": "moon sugar", "answer": "entry", "key": "honey"}, {"name": "platter", "answer": "unknown"}"#,
            r#"{"name": "moon sugar", "answer": "unknown"}"#,
            r#"{"name": "moon sugar", "answer": "unknown"}, {"name": "moon sugar", "answer": "unknown"}, {"name": "platter", "answer": "unknown"}"#,
            r#"{"name": "stardust", "answer": "unknown"}, {"name": "moon sugar", "answer": "unknown"}, {"name": "platter", "answer": "unknown"}"#,
            r#"{"name": "moon sugar", "answer": "maybe"}, {"name": "platter", "answer": "unknown"}"#,
            // No calories, implausible calories, an implausible cup.
            r#"{"name": "moon sugar", "answer": "estimate"}, {"name": "platter", "answer": "unknown"}"#,
            r#"{"name": "moon sugar", "answer": "estimate", "kcal_per_100g": 2000}, {"name": "platter", "answer": "unknown"}"#,
            r#"{"name": "moon sugar", "answer": "estimate", "kcal_per_100g": 300, "grams_per_cup": 5000}, {"name": "platter", "answer": "unknown"}"#,
        ] {
            let json = with_cheese(rest);
            assert!(validate(&names(), response(&json)).is_err(), "{json}");
        }
        // An ambiguous name is never estimated or called not food.
        for cheese in [
            r#"{"name": "cheese", "answer": "estimate", "kcal_per_100g": 400}"#,
            r#"{"name": "cheese", "answer": "not_food"}"#,
        ] {
            let json = format!(
                r#"{{"resolutions": [{{"name": "moon sugar", "answer": "unknown"}}, {{"name": "platter", "answer": "unknown"}}, {cheese}]}}"#
            );
            assert!(validate(&names(), response(&json)).is_err(), "{json}");
        }
    }
}
