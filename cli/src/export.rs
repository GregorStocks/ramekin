use anyhow::{Context, Result};
use ramekin_client::apis::auth_api;
use ramekin_client::apis::configuration::Configuration;
use ramekin_client::models::LoginRequest;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Export a single recipe to a .paprikarecipe file
pub async fn export_recipe(
    server: &str,
    username: &str,
    password: &str,
    recipe_id: &str,
    output_path: &Path,
) -> Result<()> {
    // Authenticate
    let mut config = Configuration::new();
    config.base_path = server.to_string();

    let login_response = auth_api::login(
        &config,
        LoginRequest {
            username: username.to_string(),
            password: password.to_string(),
        },
    )
    .await
    .context("Failed to login")?;

    config.bearer_access_token = Some(login_response.token.clone());

    // Download the exported recipe via HTTP client directly
    // (the generated client doesn't handle binary responses well)
    let client = reqwest::Client::new();
    let url = format!("{}/api/recipes/{}/export", server, recipe_id);

    let response = client
        .get(&url)
        .bearer_auth(&login_response.token)
        .send()
        .await
        .context("Failed to send export request")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Export failed with status {}: {}", status, body);
    }

    let bytes = response
        .bytes()
        .await
        .context("Failed to read response body")?;

    let mut file = File::create(output_path)
        .with_context(|| format!("Failed to create file: {}", output_path.display()))?;

    file.write_all(&bytes)
        .with_context(|| format!("Failed to write to file: {}", output_path.display()))?;

    println!(
        "Exported recipe to: {} ({} bytes)",
        output_path.display(),
        bytes.len()
    );

    Ok(())
}

/// Export all recipes to a .paprikarecipes file
pub async fn export_all(
    server: &str,
    username: &str,
    password: &str,
    output_path: &Path,
) -> Result<()> {
    // Authenticate
    let mut config = Configuration::new();
    config.base_path = server.to_string();

    let login_response = auth_api::login(
        &config,
        LoginRequest {
            username: username.to_string(),
            password: password.to_string(),
        },
    )
    .await
    .context("Failed to login")?;

    // Download the exported recipes via HTTP client directly
    let client = reqwest::Client::new();
    let url = format!("{}/api/recipes/export", server);

    println!("Exporting all recipes... (this may take a while)");

    let response = client
        .get(&url)
        .bearer_auth(&login_response.token)
        .send()
        .await
        .context("Failed to send export request")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Export failed with status {}: {}", status, body);
    }

    let bytes = response
        .bytes()
        .await
        .context("Failed to read response body")?;

    let mut file = File::create(output_path)
        .with_context(|| format!("Failed to create file: {}", output_path.display()))?;

    file.write_all(&bytes)
        .with_context(|| format!("Failed to write to file: {}", output_path.display()))?;

    println!(
        "Exported all recipes to: {} ({} bytes)",
        output_path.display(),
        bytes.len()
    );

    Ok(())
}
