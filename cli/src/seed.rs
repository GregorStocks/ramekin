use crate::import;
use anyhow::{Context, Result};
use ramekin_client::apis::auth_api;
use ramekin_client::apis::configuration::Configuration;
use ramekin_client::models::{LoginRequest, SignupRequest};
use std::path::Path;

pub async fn seed(server: &str, username: &str, password: &str, file: &Path) -> Result<()> {
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
    auth_api::signup(
        &config,
        SignupRequest {
            username: username.to_string(),
            password: password.to_string(),
        },
    )
    .await
    .context("Failed to create user")?;

    println!("Created user '{}'", username);

    // Import recipes from file
    import::import(server, username, password, file).await
}
