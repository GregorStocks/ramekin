use crate::import;
use anyhow::{Context, Result};
use ramekin_client::apis::configuration::Configuration;
use ramekin_client::apis::tags_api::CreateTagError;
use ramekin_client::apis::{auth_api, tags_api};
use ramekin_client::models::{CreateTagRequest, LoginRequest, SignupRequest};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct TagsFile {
    tags: Vec<String>,
}

/// Seed progress for one server and user, so a rerun after an interrupted or
/// failed seed resumes the import instead of skipping it or duplicating recipes.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct SeedState {
    server: String,
    username: String,
    complete: bool,
    /// Archive entry name -> import job submitted for it.
    jobs: HashMap<String, uuid::Uuid>,
}

impl SeedState {
    fn new(server: &str, username: &str) -> Self {
        Self {
            server: server.to_string(),
            username: username.to_string(),
            ..Default::default()
        }
    }

    /// The recorded state for this server and user, if any. A file for a
    /// different seed target is no record of this one.
    fn load(path: &Path, server: &str, username: &str) -> Result<Option<Self>> {
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("Failed to read seed state {}", path.display()))
            }
        };
        let state: Self = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse seed state {}", path.display()))?;
        Ok((state.server == server && state.username == username).then_some(state))
    }

    /// Write via a temp file and rename so an interrupted save can't corrupt it.
    fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create {}", dir.display()))?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)
            .with_context(|| format!("Failed to write {}", tmp.display()))?;
        std::fs::rename(&tmp, path)
            .with_context(|| format!("Failed to write seed state {}", path.display()))
    }
}

pub async fn seed(
    server: &str,
    username: &str,
    password: &str,
    tags_file: Option<&Path>,
    preserve_tags: bool,
    file: &Path,
    state_file: &Path,
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

    let (token, mut state) = match login_result {
        Ok(login_response) => match SeedState::load(state_file, server, username)? {
            None => {
                tracing::info!(
                    "User '{}' already exists with no seed record in {}, skipping seed",
                    username,
                    state_file.display()
                );
                return Ok(());
            }
            Some(state) if state.complete => {
                tracing::info!("User '{}' already seeded, skipping seed", username);
                return Ok(());
            }
            Some(state) => {
                tracing::info!(
                    "Resuming incomplete seed for user '{}' ({} recipe(s) previously submitted)",
                    username,
                    state.jobs.len()
                );
                (login_response.token, state)
            }
        },
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
            // Any existing state belongs to a user that no longer exists.
            let state = SeedState::new(server, username);
            state.save(state_file)?;
            (login_response.token, state)
        }
    };

    config.bearer_access_token = Some(token);

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

    // Import recipes from file, recording each submission before the next one
    let previous = std::mem::take(&mut state.jobs);
    let imported =
        import::import_archive(&config, preserve_tags, file, &previous, |entry, job_id| {
            state.jobs.insert(entry.to_string(), job_id);
            state.save(state_file)
        })
        .await;
    // Keep previous jobs that were waited on rather than resubmitted.
    for (entry, job_id) in previous {
        state.jobs.entry(entry).or_insert(job_id);
    }
    if imported.is_ok() {
        state.complete = true;
    }
    state.save(state_file)?;
    imported
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

    #[test]
    fn state_round_trips_only_for_its_own_target() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("logs/seed-state.json");
        assert_eq!(SeedState::load(&path, "http://s", "t").unwrap(), None);

        let mut state = SeedState::new("http://s", "t");
        state
            .jobs
            .insert("Soup.paprikarecipe".into(), uuid::Uuid::new_v4());
        state.save(&path).unwrap();

        assert_eq!(
            SeedState::load(&path, "http://s", "t").unwrap(),
            Some(state)
        );
        assert_eq!(SeedState::load(&path, "http://other", "t").unwrap(), None);
        assert_eq!(SeedState::load(&path, "http://s", "u").unwrap(), None);
    }
}
