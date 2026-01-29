use crate::import;
use anyhow::{Context, Result};
use ramekin_client::apis::configuration::Configuration;
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
        println!("User '{}' already exists, skipping seed", username);
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

    println!("Created user '{}'", username);

    // Set up authenticated config for tag creation
    config.bearer_access_token = Some(login_response.token);

    // Create tags from file if provided
    if let Some(tags_path) = tags_file {
        let tags_content =
            std::fs::read_to_string(tags_path).context("Failed to read tags file")?;
        let tags_data: TagsFile =
            serde_json::from_str(&tags_content).context("Failed to parse tags file")?;

        println!("Creating {} tags...", tags_data.tags.len());
        for tag_name in &tags_data.tags {
            match tags_api::create_tag(
                &config,
                CreateTagRequest {
                    name: tag_name.clone(),
                },
            )
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    // Ignore 409 conflicts (tag already exists)
                    let is_conflict = matches!(&e, ramekin_client::apis::Error::ResponseError(resp) if resp.status == reqwest::StatusCode::CONFLICT);
                    if !is_conflict {
                        tracing::warn!(tag = %tag_name, error = %e, "Failed to create tag");
                    }
                }
            }
        }
        println!("Tags created");
    }

    // Import recipes from file
    import::import(server, username, password, preserve_tags, file).await
}
