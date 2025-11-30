use anyhow::{Context, Result};
use base64::Engine;
use flate2::read::GzDecoder;
use ramekin_client::apis::configuration::Configuration;
use ramekin_client::apis::{auth_api, recipes_api};
use ramekin_client::models::{CreateRecipeRequest, Ingredient, LoginRequest};
use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

/// Paprika recipe format (subset of fields we care about)
#[derive(Debug, Deserialize)]
struct PaprikaRecipe {
    name: String,
    ingredients: Option<String>,
    directions: Option<String>,
    description: Option<String>,
    #[allow(dead_code)]
    notes: Option<String>,
    source: Option<String>,
    source_url: Option<String>,
    categories: Option<Vec<String>>,
    photo_data: Option<String>,
    #[serde(default)]
    photos: Vec<PaprikaPhoto>,
}

#[derive(Debug, Deserialize)]
struct PaprikaPhoto {
    data: Option<String>,
}

/// Upload a photo via multipart form and return its UUID
async fn upload_photo(config: &Configuration, image_data: &[u8]) -> Result<uuid::Uuid> {
    let client = reqwest::Client::new();

    let part = reqwest::multipart::Part::bytes(image_data.to_vec())
        .file_name("image.jpg")
        .mime_str("image/jpeg")?;

    let form = reqwest::multipart::Form::new().part("file", part);

    let mut request = client
        .post(format!("{}/api/photos", config.base_path))
        .multipart(form);

    if let Some(ref token) = config.bearer_access_token {
        request = request.bearer_auth(token);
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Photo upload failed with status {}: {}", status, body);
    }

    #[derive(Deserialize)]
    struct UploadResponse {
        id: uuid::Uuid,
    }

    let upload_response: UploadResponse = response.json().await?;
    Ok(upload_response.id)
}

/// Parse ingredients from Paprika's newline-separated format into structured ingredients.
/// For now, we just put the whole line as the item since parsing ingredient strings
/// (e.g. "1 1/2 cups flour, sifted") is complex and error-prone.
fn parse_ingredients(ingredients_str: &str) -> Vec<Ingredient> {
    ingredients_str
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| Ingredient {
            item: line.trim().to_string(),
            amount: None,
            unit: None,
            note: None,
        })
        .collect()
}

/// Convert a Paprika recipe to our API format
fn convert_recipe(recipe: &PaprikaRecipe, photo_id: Option<uuid::Uuid>) -> CreateRecipeRequest {
    let ingredients = recipe
        .ingredients
        .as_ref()
        .map(|s| parse_ingredients(s))
        .unwrap_or_default();

    // Use directions as instructions, fall back to empty string
    let instructions = recipe.directions.clone().unwrap_or_default();

    CreateRecipeRequest {
        title: recipe.name.clone(),
        description: recipe.description.clone().map(Some),
        instructions,
        ingredients,
        tags: recipe.categories.clone().map(Some),
        source_name: recipe.source.clone().map(Some),
        source_url: recipe.source_url.clone().map(Some),
        photo_ids: photo_id.map(|id| Some(vec![id])),
    }
}

pub async fn import(server: &str, username: &str, password: &str, file_path: &Path) -> Result<()> {
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

    config.bearer_access_token = Some(login_response.token);

    // Open the paprikarecipes file
    let file = File::open(file_path)
        .with_context(|| format!("Failed to open file: {}", file_path.display()))?;

    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("Failed to read zip archive: {}", file_path.display()))?;

    println!("Found {} recipes in archive", archive.len());

    let mut success_count = 0;
    let mut error_count = 0;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let entry_name = entry.name().to_string();

        if !entry_name.ends_with(".paprikarecipe") {
            println!("  Skipping non-recipe file: {}", entry_name);
            continue;
        }

        // Read the gzipped content
        let mut compressed_data = Vec::new();
        entry.read_to_end(&mut compressed_data)?;

        // Decompress with gzip
        let mut decoder = GzDecoder::new(&compressed_data[..]);
        let mut json_content = String::new();
        decoder
            .read_to_string(&mut json_content)
            .with_context(|| format!("Failed to decompress recipe: {}", entry_name))?;

        // Parse the recipe JSON
        let recipe: PaprikaRecipe = serde_json::from_str(&json_content)
            .with_context(|| format!("Failed to parse recipe JSON: {}", entry_name))?;

        let recipe_name = recipe.name.clone();

        // Try to upload the photo if present
        let photo_id = if let Some(ref photo_data) = recipe.photo_data {
            if !photo_data.is_empty() {
                match base64::engine::general_purpose::STANDARD.decode(photo_data) {
                    Ok(image_bytes) => match upload_photo(&config, &image_bytes).await {
                        Ok(id) => Some(id),
                        Err(e) => {
                            println!(
                                "  Warning: Failed to upload photo for '{}': {}",
                                recipe_name, e
                            );
                            None
                        }
                    },
                    Err(e) => {
                        println!(
                            "  Warning: Failed to decode photo for '{}': {}",
                            recipe_name, e
                        );
                        None
                    }
                }
            } else {
                None
            }
        } else if let Some(photo) = recipe.photos.first() {
            // Try photos array as fallback
            if let Some(ref data) = photo.data {
                if !data.is_empty() {
                    match base64::engine::general_purpose::STANDARD.decode(data) {
                        Ok(image_bytes) => match upload_photo(&config, &image_bytes).await {
                            Ok(id) => Some(id),
                            Err(e) => {
                                println!(
                                    "  Warning: Failed to upload photo for '{}': {}",
                                    recipe_name, e
                                );
                                None
                            }
                        },
                        Err(e) => {
                            println!(
                                "  Warning: Failed to decode photo for '{}': {}",
                                recipe_name, e
                            );
                            None
                        }
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Convert and create the recipe
        let request = convert_recipe(&recipe, photo_id);

        match recipes_api::create_recipe(&config, request).await {
            Ok(_) => {
                println!("  Imported: {}", recipe_name);
                success_count += 1;
            }
            Err(e) => {
                println!("  Error importing '{}': {}", recipe_name, e);
                error_count += 1;
            }
        }
    }

    println!();
    println!("{}", "=".repeat(50));
    println!("IMPORT COMPLETE");
    println!("{}", "=".repeat(50));
    println!("Successful: {}", success_count);
    println!("Errors: {}", error_count);
    println!("{}", "=".repeat(50));

    Ok(())
}
