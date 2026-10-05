use crate::import;
use anyhow::{Context, Result};
use ramekin_client::apis::configuration::Configuration;
use ramekin_client::apis::tags_api::CreateTagError;
use ramekin_client::apis::{auth_api, tags_api};
use ramekin_client::models::{CreateTagRequest, LoginRequest, SignupRequest};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct TagsFile {
    tags: Vec<String>,
}

pub async fn seed(
    server: &str,
    username: &str,
    password: &str,
    tags_file: Option<&Path>,
    preserve_tags: bool,
    file: &Path,
) -> Result<()> {
    let mut config = Configuration::new();
    config.base_path = server.to_string();

    // Try to login first - if user exists, we're done
    let login_result = auth_api::login(
        &config,
        LoginRequest {
            username: username.to_string(),
            password: password.to_string(),
        },
    )
    .await;

    if login_result.is_ok() {
        tracing::info!("User '{}' already exists, skipping seed", username);
        return Ok(());
    }

    // User doesn't exist, create them
    let login_response = auth_api::signup(
        &config,
        SignupRequest {
            username: username.to_string(),
            password: password.to_string(),
        },
    )
    .await
    .context("Failed to create user")?;

    tracing::info!("Created user '{}'", username);

    // Set up authenticated config for tag creation
    config.bearer_access_token = Some(login_response.token);

    // Create tags from file if provided
    if let Some(tags_path) = tags_file {
        let tags_content =
            std::fs::read_to_string(tags_path).context("Failed to read tags file")?;
        let tags_data: TagsFile =
            serde_json::from_str(&tags_content).context("Failed to parse tags file")?;

        tracing::info!("Creating {} tags...", tags_data.tags.len());
        for tag_name in &tags_data.tags {
            let result = tags_api::create_tag(
                &config,
                CreateTagRequest {
                    name: tag_name.clone(),
                },
            )
            .await;
            match result {
                Err(e) if is_tag_conflict(&e) => {
                    tracing::debug!(tag = %tag_name, "Tag already exists");
                }
                result => {
                    result.with_context(|| {
                        format!(
                            "Failed to create tag '{tag_name}'. User '{username}' was already \
                             created, so rerunning seed will skip it; use a new username or \
                             run `import` directly"
                        )
                    })?;
                }
            }
        }
        tracing::info!("Tags created");
    }

    // Import recipes from file
    import::import(server, username, password, preserve_tags, file).await
}

/// Creating a tag that already exists is expected when seeding; anything else is a real failure.
fn is_tag_conflict(error: &ramekin_client::apis::Error<CreateTagError>) -> bool {
    matches!(error, ramekin_client::apis::Error::ResponseError(resp) if resp.status == reqwest::StatusCode::CONFLICT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramekin_client::apis::{Error, ResponseContent};

    fn response_error(status: reqwest::StatusCode) -> Error<CreateTagError> {
        Error::ResponseError(ResponseContent {
            status,
            content: String::new(),
            entity: None,
        })
    }

    #[test]
    fn only_conflicts_are_ignored() {
        assert!(is_tag_conflict(&response_error(
            reqwest::StatusCode::CONFLICT
        )));
        assert!(!is_tag_conflict(&response_error(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR
        )));
    }
}
