use thiserror::Error;

#[derive(Error, Debug)]
pub enum FetchError {
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Invalid response encoding: {0}")]
    InvalidEncoding(String),
}

#[derive(Error, Debug)]
pub enum ExtractError {
    #[error("No Recipe found in JSON-LD")]
    NoRecipe,

    #[error("Invalid JSON-LD: {0}")]
    InvalidJson(String),

    #[error("Missing required field: {0}")]
    MissingField(String),
}
