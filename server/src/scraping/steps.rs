//! Server-specific pipeline step implementations.
//!
//! These implement the `PipelineStep` trait with database operations
//! for storing recipes, fetching images, etc.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use diesel::prelude::*;
use serde_json::json;
use uuid::Uuid;

use ramekin_core::pipeline::{
    steps::{FetchImagesStepMeta, SaveRecipeStepMeta},
    PipelineStep, StepContext, StepMetadata, StepResult,
};
use ramekin_core::{FailedImageFetch, FetchImagesOutput, RawRecipe};

use crate::db::DbPool;
use crate::models::{Ingredient, NewPhoto, NewRecipe, NewRecipeVersion};
use crate::photos::processing::{process_image, MAX_FILE_SIZE};
use crate::schema::{photos, recipe_versions, recipes};

use super::is_host_allowed;

/// Server implementation of FetchHtml step.
///
/// Uses ramekin_core::fetch_html directly (no caching).
pub struct FetchHtmlStep;

impl FetchHtmlStep {
    pub const NAME: &'static str = "fetch_html";
}

#[async_trait]
impl PipelineStep for FetchHtmlStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Fetch HTML from URL",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Check host allowlist
        if let Err(e) = is_host_allowed(ctx.url) {
            return StepResult {
                step_name: Self::NAME.to_string(),
                success: false,
                output: serde_json::Value::Null,
                error: Some(e.to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: None,
            };
        }

        match ramekin_core::fetch_html(ctx.url).await {
            Ok(html) => StepResult {
                step_name: Self::NAME.to_string(),
                success: true,
                output: json!({ "html": html }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: Some("extract_recipe".to_string()),
            },
            Err(e) => StepResult {
                step_name: Self::NAME.to_string(),
                success: false,
                output: serde_json::Value::Null,
                error: Some(e.to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: None,
            },
        }
    }
}

/// Server implementation of FetchImages step.
///
/// Fetches images from URLs, processes them, and stores as Photo records in the database.
pub struct FetchImagesStep {
    pool: Arc<DbPool>,
    user_id: Uuid,
}

impl FetchImagesStep {
    pub fn new(pool: Arc<DbPool>, user_id: Uuid) -> Self {
        Self { pool, user_id }
    }
}

#[async_trait]
impl PipelineStep for FetchImagesStep {
    fn metadata(&self) -> StepMetadata {
        FetchImagesStepMeta::metadata()
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Get extract output to find image URLs
        let extract_output = match ctx.outputs.get_output("extract_recipe") {
            Some(o) => o,
            None => {
                return StepResult {
                    step_name: FetchImagesStepMeta::NAME.to_string(),
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some("extract_recipe output not found".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Parse raw_recipe to get image URLs
        let raw_recipe: RawRecipe = match extract_output
            .get("raw_recipe")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
        {
            Some(r) => r,
            None => {
                return StepResult {
                    step_name: FetchImagesStepMeta::NAME.to_string(),
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some("No raw_recipe in extract output".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Fetch images (only the first one for now)
        let mut photo_ids = Vec::new();
        let mut failed_urls = Vec::new();

        if let Some(url) = raw_recipe.image_urls.first() {
            match self.fetch_and_store_image(url).await {
                Ok(photo_id) => photo_ids.push(photo_id),
                Err(e) => {
                    tracing::warn!("Failed to fetch image {}: {}", url, e);
                    failed_urls.push(FailedImageFetch {
                        url: url.clone(),
                        error: e,
                    });
                }
            }
        }

        let output = FetchImagesOutput {
            photo_ids,
            failed_urls,
        };

        StepResult {
            step_name: FetchImagesStepMeta::NAME.to_string(),
            success: true, // Image fetch failures don't fail the pipeline
            output: serde_json::to_value(&output).unwrap_or_default(),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: Some("save_recipe".to_string()),
        }
    }
}

impl FetchImagesStep {
    async fn fetch_and_store_image(&self, url: &str) -> Result<Uuid, String> {
        // Check host allowlist
        is_host_allowed(url).map_err(|e| e.to_string())?;

        // Fetch the image bytes
        let data = ramekin_core::fetch_bytes(url)
            .await
            .map_err(|e| e.to_string())?;

        // Validate size
        if data.len() > MAX_FILE_SIZE {
            return Err(format!(
                "Image too large: {} bytes (max {})",
                data.len(),
                MAX_FILE_SIZE
            ));
        }

        // Process: validate format, generate thumbnail
        let (content_type, thumbnail) = process_image(&data).map_err(|e| e.to_string())?;

        // Store in database
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        let new_photo = NewPhoto {
            user_id: self.user_id,
            content_type: &content_type,
            data: &data,
            thumbnail: &thumbnail,
        };

        let photo_id: Uuid = diesel::insert_into(photos::table)
            .values(&new_photo)
            .returning(photos::id)
            .get_result(&mut conn)
            .map_err(|e| e.to_string())?;

        tracing::info!("Stored photo {} from {}", photo_id, url);
        Ok(photo_id)
    }
}

/// Server implementation of SaveRecipe step.
///
/// Creates a recipe and recipe_version in the database, or updates an existing
/// recipe if `existing_recipe_id` is set (for rescrape).
pub struct SaveRecipeStep {
    pool: Arc<DbPool>,
    user_id: Uuid,
    existing_recipe_id: Option<Uuid>,
}

impl SaveRecipeStep {
    pub fn new(pool: Arc<DbPool>, user_id: Uuid) -> Self {
        Self {
            pool,
            user_id,
            existing_recipe_id: None,
        }
    }

    pub fn for_rescrape(pool: Arc<DbPool>, user_id: Uuid, recipe_id: Uuid) -> Self {
        Self {
            pool,
            user_id,
            existing_recipe_id: Some(recipe_id),
        }
    }
}

#[async_trait]
impl PipelineStep for SaveRecipeStep {
    fn metadata(&self) -> StepMetadata {
        SaveRecipeStepMeta::metadata()
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Get extract output
        let extract_output = match ctx.outputs.get_output("extract_recipe") {
            Some(o) => o,
            None => {
                return StepResult {
                    step_name: SaveRecipeStepMeta::NAME.to_string(),
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some("extract_recipe output not found".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Parse raw_recipe
        let raw_recipe: RawRecipe = match extract_output
            .get("raw_recipe")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
        {
            Some(r) => r,
            None => {
                return StepResult {
                    step_name: SaveRecipeStepMeta::NAME.to_string(),
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some("No raw_recipe in extract output".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Get photo IDs from fetch_images output
        let photo_ids: Vec<Uuid> = ctx
            .outputs
            .get_output("fetch_images")
            .and_then(|o| o.get("photo_ids").cloned())
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        // Create or update recipe in database
        let result = match self.existing_recipe_id {
            Some(recipe_id) => self.update_recipe(recipe_id, &raw_recipe, &photo_ids),
            None => self.create_recipe(&raw_recipe, &photo_ids),
        };

        match result {
            Ok(recipe_id) => StepResult {
                step_name: SaveRecipeStepMeta::NAME.to_string(),
                success: true,
                output: json!({ "recipe_id": recipe_id.to_string() }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: Some("enrich_normalize_ingredients".to_string()),
            },
            Err(e) => StepResult {
                step_name: SaveRecipeStepMeta::NAME.to_string(),
                success: false,
                output: serde_json::Value::Null,
                error: Some(e),
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: None,
            },
        }
    }
}

impl SaveRecipeStep {
    fn create_recipe(&self, raw: &RawRecipe, photo_ids: &[Uuid]) -> Result<Uuid, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        // Convert raw ingredients to our Ingredient JSON format
        let ingredients: Vec<Ingredient> = raw
            .ingredients
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| Ingredient {
                item: line.trim().to_string(),
                amount: None,
                unit: None,
                note: None,
            })
            .collect();

        let ingredients_json = serde_json::to_value(&ingredients).map_err(|e| e.to_string())?;

        // Convert photo IDs to Option<Uuid> for the database
        let photo_ids_nullable: Vec<Option<Uuid>> = photo_ids.iter().map(|id| Some(*id)).collect();

        // Use a transaction to create recipe + version atomically
        conn.transaction(|conn| {
            // 1. Create the recipe row
            let new_recipe = NewRecipe {
                user_id: self.user_id,
            };

            let recipe_id: Uuid = diesel::insert_into(recipes::table)
                .values(&new_recipe)
                .returning(recipes::id)
                .get_result(conn)?;

            // 2. Create the initial version with source='scrape'
            let new_version = NewRecipeVersion {
                recipe_id,
                title: &raw.title,
                description: raw.description.as_deref(),
                ingredients: ingredients_json.clone(),
                instructions: &raw.instructions,
                source_url: Some(&raw.source_url),
                source_name: raw.source_name.as_deref(),
                photo_ids: &photo_ids_nullable,
                tags: &[],
                servings: None,
                prep_time: None,
                cook_time: None,
                total_time: None,
                rating: None,
                difficulty: None,
                nutritional_info: None,
                notes: None,
                version_source: "scrape",
            };

            let version_id: Uuid = diesel::insert_into(recipe_versions::table)
                .values(&new_version)
                .returning(recipe_versions::id)
                .get_result(conn)?;

            // 3. Update recipe to point to this version
            diesel::update(recipes::table.find(recipe_id))
                .set(recipes::current_version_id.eq(version_id))
                .execute(conn)?;

            Ok(recipe_id)
        })
        .map_err(|e: diesel::result::Error| e.to_string())
    }

    fn update_recipe(
        &self,
        recipe_id: Uuid,
        raw: &RawRecipe,
        photo_ids: &[Uuid],
    ) -> Result<Uuid, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        // Convert raw ingredients to our Ingredient JSON format
        let ingredients: Vec<Ingredient> = raw
            .ingredients
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| Ingredient {
                item: line.trim().to_string(),
                amount: None,
                unit: None,
                note: None,
            })
            .collect();

        let ingredients_json = serde_json::to_value(&ingredients).map_err(|e| e.to_string())?;

        // Convert photo IDs to Option<Uuid> for the database
        let photo_ids_nullable: Vec<Option<Uuid>> = photo_ids.iter().map(|id| Some(*id)).collect();

        // Use a transaction to create new version and update recipe
        conn.transaction(|conn| {
            // Create a new version with source='rescrape'
            let new_version = NewRecipeVersion {
                recipe_id,
                title: &raw.title,
                description: raw.description.as_deref(),
                ingredients: ingredients_json.clone(),
                instructions: &raw.instructions,
                source_url: Some(&raw.source_url),
                source_name: raw.source_name.as_deref(),
                photo_ids: &photo_ids_nullable,
                tags: &[],
                servings: None,
                prep_time: None,
                cook_time: None,
                total_time: None,
                rating: None,
                difficulty: None,
                nutritional_info: None,
                notes: None,
                version_source: "rescrape",
            };

            let version_id: Uuid = diesel::insert_into(recipe_versions::table)
                .values(&new_version)
                .returning(recipe_versions::id)
                .get_result(conn)?;

            // Update recipe to point to this new version
            diesel::update(recipes::table.find(recipe_id))
                .set(recipes::current_version_id.eq(version_id))
                .execute(conn)?;

            Ok(recipe_id)
        })
        .map_err(|e: diesel::result::Error| e.to_string())
    }
}

/// Server implementation of ApplyAutoTags step.
///
/// Takes the suggested tags from enrich_auto_tag output and creates a new
/// recipe version with those tags applied.
pub struct ApplyAutoTagsStep {
    pool: Arc<DbPool>,
}

impl ApplyAutoTagsStep {
    pub const NAME: &'static str = "apply_auto_tags";

    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PipelineStep for ApplyAutoTagsStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Apply auto-suggested tags to recipe",
            continues_on_failure: true, // Don't fail the pipeline if tags can't be applied
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Get recipe_id from save_recipe output
        let save_output = ctx.outputs.get_output("save_recipe");
        let recipe_id = match save_output
            .as_ref()
            .and_then(|o| o.get("recipe_id"))
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        {
            Some(id) => id,
            None => {
                return StepResult {
                    step_name: Self::NAME.to_string(),
                    success: false,
                    output: json!({ "error": "No recipe_id in save_recipe output" }),
                    error: Some("No recipe_id in save_recipe output".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: Some("enrich_generate_photo".to_string()),
                };
            }
        };

        // Get suggested_tags from enrich_auto_tag output
        let auto_tag_output = ctx.outputs.get_output("enrich_auto_tag");
        let suggested_tags: Vec<String> = auto_tag_output
            .as_ref()
            .and_then(|o| o.get("suggested_tags"))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        // If no tags suggested, nothing to do
        if suggested_tags.is_empty() {
            return StepResult {
                step_name: Self::NAME.to_string(),
                success: true,
                output: json!({ "message": "No tags to apply", "tags_applied": [] }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: Some("enrich_generate_photo".to_string()),
            };
        }

        // Apply the tags to the recipe
        match self.apply_tags(recipe_id, &suggested_tags) {
            Ok(version_id) => StepResult {
                step_name: Self::NAME.to_string(),
                success: true,
                output: json!({
                    "tags_applied": suggested_tags,
                    "new_version_id": version_id.to_string(),
                }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: Some("enrich_generate_photo".to_string()),
            },
            Err(e) => StepResult {
                step_name: Self::NAME.to_string(),
                success: false,
                output: json!({ "error": e }),
                error: Some(e),
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: Some("enrich_generate_photo".to_string()),
            },
        }
    }
}

impl ApplyAutoTagsStep {
    fn apply_tags(&self, recipe_id: Uuid, tags: &[String]) -> Result<Uuid, String> {
        use crate::models::{NewRecipeVersion, Recipe, RecipeVersion};

        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        // Get the current version
        let recipe: Recipe = recipes::table
            .find(recipe_id)
            .first(&mut conn)
            .map_err(|e| e.to_string())?;

        let current_version_id = recipe
            .current_version_id
            .ok_or("Recipe has no current version")?;

        let current_version: RecipeVersion = recipe_versions::table
            .find(current_version_id)
            .first(&mut conn)
            .map_err(|e| e.to_string())?;

        // Merge existing tags with new tags (avoid duplicates, case-insensitive)
        let existing_tags: Vec<String> = current_version
            .tags
            .iter()
            .filter_map(|t| t.clone())
            .collect();

        let mut merged_tags = existing_tags.clone();
        for tag in tags {
            if !merged_tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                merged_tags.push(tag.clone());
            }
        }

        // Convert to Option<String> for the database
        let tags_for_db: Vec<Option<String>> = merged_tags.into_iter().map(Some).collect();

        // Create new version with tags
        conn.transaction(|conn| {
            let new_version = NewRecipeVersion {
                recipe_id,
                title: &current_version.title,
                description: current_version.description.as_deref(),
                ingredients: current_version.ingredients.clone(),
                instructions: &current_version.instructions,
                source_url: current_version.source_url.as_deref(),
                source_name: current_version.source_name.as_deref(),
                photo_ids: &current_version.photo_ids,
                tags: &tags_for_db,
                servings: current_version.servings.as_deref(),
                prep_time: current_version.prep_time.as_deref(),
                cook_time: current_version.cook_time.as_deref(),
                total_time: current_version.total_time.as_deref(),
                rating: current_version.rating,
                difficulty: current_version.difficulty.as_deref(),
                nutritional_info: current_version.nutritional_info.as_deref(),
                notes: current_version.notes.as_deref(),
                version_source: "auto_tag",
            };

            let version_id: Uuid = diesel::insert_into(recipe_versions::table)
                .values(&new_version)
                .returning(recipe_versions::id)
                .get_result(conn)?;

            // Update recipe to point to new version
            diesel::update(recipes::table.find(recipe_id))
                .set(recipes::current_version_id.eq(version_id))
                .execute(conn)?;

            Ok(version_id)
        })
        .map_err(|e: diesel::result::Error| e.to_string())
    }
}

// Enrich steps use generic implementations from ramekin-core
// (EnrichNormalizeIngredientsStep, EnrichAutoTagStep, EnrichGeneratePhotoStep)
