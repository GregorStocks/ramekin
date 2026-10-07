use crate::import;
use anyhow::{Context, Result};
use ramekin_client::apis::configuration::Configuration;
use ramekin_client::apis::tags_api::CreateTagError;
use ramekin_client::apis::{auth_api, recipes_api, tags_api};
use ramekin_client::models::{CreateTagRequest, LoginRequest, SignupRequest};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct TagsFile {
    tags: Vec<String>,
}

/// Seed imports are keyed by archive entry so a rerun after an interrupted or
/// failed seed resumes it without duplicating recipes. Hashed so long entry
/// names stay within the server's key length limit.
fn seed_key(entry_name: &str) -> String {
    let digest = Sha256::digest(entry_name.as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("seed:{hex}")
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

    let login_result = auth_api::login(
        &config,
        LoginRequest {
            username: username.to_string(),
            password: password.to_string(),
        },
    )
    .await;

    let (token, user_existed) = match login_result {
        Ok(login_response) => (login_response.token, true),
        Err(_) => {
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
            (login_response.token, false)
        }
    };
    config.bearer_access_token = Some(token);

    let entries = import::recipe_entry_names(file)?;
    let mut by_key = import::lookup_import_jobs(
        &config,
        entries.iter().map(|entry| seed_key(entry)).collect(),
    )
    .await?;
    let existing: HashMap<String, uuid::Uuid> = entries
        .iter()
        .filter_map(|entry| Some((entry.clone(), by_key.remove(&seed_key(entry))?)))
        .collect();

    if user_existed && existing.is_empty() && has_recipes(&config).await? {
        tracing::info!(
            "User '{}' already has recipes from a seed without import keys, skipping seed",
            username
        );
        return Ok(());
    }
    if user_existed {
        tracing::info!(
            "User '{}' already exists; {} of {} seed recipe(s) already submitted",
            username,
            existing.len(),
            entries.len()
        );
    }

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
                    result.with_context(|| format!("Failed to create tag '{tag_name}'"))?;
                }
            }
        }
        tracing::info!("Tags created");
    }

    // Import recipes from file
    import::import_archive(&config, preserve_tags, file, Some(&seed_key), &existing).await
}

async fn has_recipes(config: &Configuration) -> Result<bool> {
    let response = recipes_api::list_recipes(config, Some(1), None, None, None, None)
        .await
        .context("Failed to list recipes")?;
    Ok(response.pagination.total > 0)
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
    fn seed_keys_are_bounded_and_distinct() {
        let long = "x".repeat(1000);
        assert_eq!(seed_key(&long).len(), seed_key("Soup.paprikarecipe").len());
        assert!(seed_key(&long).len() <= 255);
        assert_ne!(
            seed_key("Soup.paprikarecipe"),
            seed_key("Bread.paprikarecipe")
        );
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
