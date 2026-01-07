use anyhow::{Context, Result};
use ramekin_client::apis::auth_api;
use ramekin_client::apis::configuration::Configuration;
use ramekin_client::models::LoginRequest;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Helper to make a binary GET request using the Configuration's client and auth.
/// The generated client doesn't handle binary responses (returns `()` and discards body),
/// so we use this workaround while still reusing the Configuration's client and auth.
async fn binary_get(config: &Configuration, path: &str) -> Result<Vec<u8>> {
    let url = format!("{}{}", config.base_path, path);
    let mut req = config.client.get(&url);

    if let Some(ref token) = config.bearer_access_token {
        req = req.bearer_auth(token);
    }

    let response = req.send().await.context("Failed to send request")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Request failed with status {}: {}", status, body);
    }

    let bytes = response
        .bytes()
        .await
        .context("Failed to read response body")?;

    Ok(bytes.to_vec())
}

/// Export a single recipe to a .paprikarecipe file
pub async fn export_recipe(
    server: &str,
    username: &str,
    password: &str,
    recipe_id: &str,
    output_path: &Path,
) -> Result<()> {
    // Authenticate using the generated client
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

    config.bearer_access_token = Some(login_response.token);

    // Download the exported recipe using the config's client
    let bytes = binary_get(&config, &format!("/api/recipes/{}/export", recipe_id)).await?;

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
    // Authenticate using the generated client
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

    config.bearer_access_token = Some(login_response.token);

    println!("Exporting all recipes... (this may take a while)");

    // Download the exported recipes using the config's client
    let bytes = binary_get(&config, "/api/recipes/export").await?;

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
