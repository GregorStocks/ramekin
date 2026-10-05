//! AI configuration from environment variables.

use std::env;
use thiserror::Error;

/// Default OpenRouter base URL.
pub const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";

/// Default model for tags, titles, descriptions and custom enrich, chosen
/// with `make ai-eval` (data/ai-evals/): judged good as often as any model on
/// titles and descriptions, tied best on custom enrich at under half
/// claude-sonnet-5.5's cost, and well ahead of gemini-2.5-flash on tags.
pub const DEFAULT_MODEL: &str = "google/gemini-3.8-flash";
/// Default model for ingredient names and weights, chosen with `make ai-eval`
/// (data/ai-evals/): the most accurate on all three ingredient suites once its
/// fenced JSON is accepted, at about $0.002 per item.
pub const DEFAULT_INGREDIENT_MODEL: &str = "anthropic/claude-opus-5.5";
/// Default model for recipe extraction from pasted text and photos
/// (`for_extraction`), chosen with `make ai-eval` (data/ai-evals/): exact on
/// both suites, the fastest of the exact models (10-15 s a recipe), at about
/// $0.02 a recipe.
pub const DEFAULT_EXTRACTION_MODEL: &str = "anthropic/claude-sonnet-5.5";
/// Default model for recipe photos, chosen with `make ai-eval SUITE=recipe-photos`
/// over the leaderboard leaders: judged best most often, with no refusals or
/// timeouts, at about $0.07 a photo.
pub const DEFAULT_IMAGE_MODEL: &str = "google/gemini-3.1-flash-image";

/// Default rate limit between requests in milliseconds.
pub const DEFAULT_RATE_LIMIT_MS: u64 = 500;
/// Default request timeout in seconds.
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
/// The least request timeout for ingredient calls. They run in the
/// background, and a batch of 40 takes the ingredient model 20-40 s.
pub const INGREDIENT_TIMEOUT_SECS: u64 = 120;
/// The least request timeout for extraction calls, which write out a whole
/// recipe: 10-20 s for a long one. Text import waits on it in the request, so
/// it stays within the clients' 60 s.
pub const EXTRACTION_TIMEOUT_SECS: u64 = 60;
/// The least request timeout for generating a recipe photo: 10-25 s for the
/// current image models (FLUX 3 about 20 s). The user waits on it in the
/// request, so it stays within the clients' 60 s, as extraction does.
pub const IMAGE_TIMEOUT_SECS: u64 = 60;

#[derive(Error, Debug, Clone)]
pub enum ConfigError {
    #[error("Missing required environment variable: {0}")]
    MissingEnvVar(String),
    #[error(
        "OPENROUTER_API_KEY contains the example placeholder; set a real key or leave it unset"
    )]
    PlaceholderApiKey,
}

/// AI client configuration.
#[derive(Debug, Clone)]
pub struct AiConfig {
    /// API key for OpenRouter.
    pub api_key: String,
    /// Model name for tags, titles, descriptions and custom enrich.
    pub model: String,
    /// Model name for image generation.
    pub image_model: String,
    /// Image quality to ask for (`auto`, `low`, `medium`, `high`), or the
    /// model's default. Models without the setting ignore it.
    pub image_quality: Option<String>,
    /// Model name for ingredient names and weights (`for_ingredients`).
    pub ingredient_model: String,
    /// Model name for recipe extraction from text and photos (`for_extraction`).
    pub extraction_model: String,
    /// Base URL for the API.
    pub base_url: String,
    /// Directory for caching responses.
    pub cache_dir: std::path::PathBuf,
    /// Milliseconds to wait between requests.
    pub rate_limit_ms: u64,
    /// Seconds before failing an API request.
    pub request_timeout_secs: u64,
}

impl AiConfig {
    /// Load configuration from environment variables.
    ///
    /// Required:
    /// - `OPENROUTER_API_KEY`: API key for OpenRouter
    ///
    /// Optional:
    /// - `RAMEKIN_AI_MODEL`: Model name (default: DEFAULT_MODEL)
    /// - `RAMEKIN_AI_IMAGE_MODEL`: Image model name (default: DEFAULT_IMAGE_MODEL)
    /// - `RAMEKIN_AI_IMAGE_QUALITY`: Image quality, auto/low/medium/high (default: the model's own)
    /// - `RAMEKIN_AI_INGREDIENT_MODEL`: Ingredient names and weights model (default: "anthropic/claude-opus-5.5")
    /// - `RAMEKIN_AI_EXTRACTION_MODEL`: Text and photo extraction model (default: DEFAULT_EXTRACTION_MODEL)
    /// - `RAMEKIN_AI_BASE_URL`: API base URL (default: "https://openrouter.ai/api/v1")
    /// - `RAMEKIN_AI_CACHE_DIR`: Cache directory (default: "~/.ramekin/ai-cache")
    /// - `RAMEKIN_AI_RATE_LIMIT_MS`: Rate limit in ms (default: 500)
    /// - `RAMEKIN_AI_TIMEOUT_SECS`: Request timeout in seconds (default: 30)
    pub fn from_env() -> Result<Self, ConfigError> {
        let api_key = validate_api_key(env::var("OPENROUTER_API_KEY"))?;

        let model = env::var("RAMEKIN_AI_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        let image_model =
            env::var("RAMEKIN_AI_IMAGE_MODEL").unwrap_or_else(|_| DEFAULT_IMAGE_MODEL.to_string());
        let image_quality = env::var("RAMEKIN_AI_IMAGE_QUALITY")
            .ok()
            .filter(|quality| !quality.trim().is_empty());
        let ingredient_model = env::var("RAMEKIN_AI_INGREDIENT_MODEL")
            .unwrap_or_else(|_| DEFAULT_INGREDIENT_MODEL.to_string());
        let extraction_model = env::var("RAMEKIN_AI_EXTRACTION_MODEL")
            .unwrap_or_else(|_| DEFAULT_EXTRACTION_MODEL.to_string());

        let base_url =
            env::var("RAMEKIN_AI_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());

        let cache_dir = env::var("RAMEKIN_AI_CACHE_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| Self::default_cache_dir());

        let rate_limit_ms = env::var("RAMEKIN_AI_RATE_LIMIT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_RATE_LIMIT_MS);

        let request_timeout_secs = env::var("RAMEKIN_AI_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_REQUEST_TIMEOUT_SECS);

        Ok(Self {
            api_key,
            model,
            image_model,
            image_quality,
            ingredient_model,
            extraction_model,
            base_url,
            cache_dir,
            rate_limit_ms,
            request_timeout_secs,
        })
    }

    /// This configuration for ingredient names and weights: their model, and
    /// a timeout long enough for a batch.
    pub fn for_ingredients(&self) -> Self {
        Self {
            model: self.ingredient_model.clone(),
            request_timeout_secs: self.request_timeout_secs.max(INGREDIENT_TIMEOUT_SECS),
            ..self.clone()
        }
    }

    /// This configuration for recipe extraction from text and photos: its
    /// model, and a timeout long enough to write out a whole recipe.
    pub fn for_extraction(&self) -> Self {
        Self {
            model: self.extraction_model.clone(),
            request_timeout_secs: self.request_timeout_secs.max(EXTRACTION_TIMEOUT_SECS),
            ..self.clone()
        }
    }

    /// Cache directory namespaced by API endpoint.
    ///
    /// Responses from the default OpenRouter endpoint live directly in
    /// `cache_dir`, preserving the existing on-disk layout. Any other endpoint
    /// (e.g. a localhost mock server in tests) gets its own subdirectory under
    /// `endpoints/`, so non-production responses can never be cached under the
    /// real model name and poison the shared cache.
    pub fn namespaced_cache_dir(&self) -> std::path::PathBuf {
        if self.base_url == DEFAULT_BASE_URL {
            return self.cache_dir.clone();
        }

        let sanitized: String = self
            .base_url
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                    c
                } else {
                    '-'
                }
            })
            .collect();

        self.cache_dir.join("endpoints").join(sanitized)
    }

    /// Get the default cache directory: ~/.ramekin/ai-cache
    pub fn default_cache_dir() -> std::path::PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".ramekin").join("ai-cache"))
            .unwrap_or_else(|| std::path::PathBuf::from("data/ai-cache"))
    }
}

fn validate_api_key(value: Result<String, env::VarError>) -> Result<String, ConfigError> {
    let key = value.map_err(|_| ConfigError::MissingEnvVar("OPENROUTER_API_KEY".to_string()))?;
    if key.trim().is_empty() {
        return Err(ConfigError::MissingEnvVar("OPENROUTER_API_KEY".to_string()));
    }
    if key.trim() == "sk-or-..." {
        return Err(ConfigError::PlaceholderApiKey);
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_or_blank_key_is_unconfigured() {
        for value in [
            Err(env::VarError::NotPresent),
            Ok(String::new()),
            Ok(" \t\n".into()),
        ] {
            assert!(matches!(
                validate_api_key(value),
                Err(ConfigError::MissingEnvVar(_))
            ));
        }
    }

    #[test]
    fn example_key_is_rejected_locally() {
        assert!(matches!(
            validate_api_key(Ok("sk-or-...".into())),
            Err(ConfigError::PlaceholderApiKey)
        ));
    }

    #[test]
    fn configured_key_is_preserved() {
        assert_eq!(
            validate_api_key(Ok("test-api-key".into())).unwrap(),
            "test-api-key"
        );
    }

    fn config_with_base_url(base_url: &str) -> AiConfig {
        AiConfig {
            api_key: "test-key".to_string(),
            model: DEFAULT_MODEL.to_string(),
            image_model: DEFAULT_IMAGE_MODEL.to_string(),
            image_quality: None,
            ingredient_model: DEFAULT_INGREDIENT_MODEL.to_string(),
            extraction_model: DEFAULT_EXTRACTION_MODEL.to_string(),
            base_url: base_url.to_string(),
            cache_dir: std::path::PathBuf::from("/tmp/ai-cache"),
            rate_limit_ms: 0,
            request_timeout_secs: 1,
        }
    }

    #[test]
    fn ingredient_calls_use_their_model_and_a_batch_length_timeout() {
        let config = config_with_base_url(DEFAULT_BASE_URL);
        let ingredients = config.for_ingredients();
        assert_eq!(ingredients.model, DEFAULT_INGREDIENT_MODEL);
        assert_eq!(ingredients.request_timeout_secs, INGREDIENT_TIMEOUT_SECS);
        // A longer configured timeout is kept.
        let patient = AiConfig {
            request_timeout_secs: 600,
            ..config
        };
        assert_eq!(patient.for_ingredients().request_timeout_secs, 600);
    }

    #[test]
    fn extraction_calls_use_their_model_and_a_recipe_length_timeout() {
        let config = config_with_base_url(DEFAULT_BASE_URL);
        let extraction = config.for_extraction();
        assert_eq!(extraction.model, DEFAULT_EXTRACTION_MODEL);
        assert_eq!(extraction.request_timeout_secs, EXTRACTION_TIMEOUT_SECS);
        let patient = AiConfig {
            request_timeout_secs: 600,
            ..config
        };
        assert_eq!(patient.for_extraction().request_timeout_secs, 600);
    }

    #[test]
    fn default_base_url_uses_cache_dir_directly() {
        let config = config_with_base_url(DEFAULT_BASE_URL);
        assert_eq!(
            config.namespaced_cache_dir(),
            std::path::PathBuf::from("/tmp/ai-cache")
        );
    }

    #[test]
    fn non_default_base_url_gets_endpoint_namespace() {
        let config = config_with_base_url("http://localhost:39123/v1");
        assert_eq!(
            config.namespaced_cache_dir(),
            std::path::PathBuf::from("/tmp/ai-cache/endpoints/http---localhost-39123-v1")
        );
    }
}
