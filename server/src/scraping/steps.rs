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
    first_scrape_auto_applied_ai_step_name, step_after_scrape_auto_applied_ai_step,
    steps::{FetchImagesStepMeta, SaveRecipeStepMeta},
    PipelineStep, StepContext, StepMetadata, StepResult,
};
use ramekin_core::{ExtractionMethod, FailedImageFetch, FetchImagesOutput, RawRecipe};

use crate::db::DbPool;
use crate::models::{Ingredient, NewPhoto, NewRecipe, NewRecipeVersion, RecipeVersionTag};
use crate::photos::processing::{process_image, MAX_FILE_SIZE};
use crate::schema::{photos, recipe_version_tags, recipe_versions, recipes};
use crate::tags::upsert_user_tag;

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
            next_step: Some("parse_ingredients".to_string()),
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
        let processed = process_image(&data).map_err(|e| e.to_string())?;

        // Store in database
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        let new_photo = NewPhoto {
            user_id: self.user_id,
            content_type: &processed.content_type,
            data: &data,
            thumbnail: &processed.thumbnail,
            width: Some(processed.width as i32),
            height: Some(processed.height as i32),
            file_size: Some(data.len() as i32),
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

/// How SaveRecipeStep should behave.
#[derive(Debug, Clone, Copy)]
pub enum SaveMode {
    /// Create a brand-new recipe.
    Create,
    /// Update an existing recipe by creating a new version from the newly
    /// scraped data.
    Rescrape(Uuid),
    /// Update an existing recipe by creating a new version that copies every
    /// field from the current version and only replaces `photo_ids` with the
    /// newly-fetched photos.
    PhotoOnly(Uuid),
}

/// Server implementation of SaveRecipe step.
pub struct SaveRecipeStep {
    pool: Arc<DbPool>,
    user_id: Uuid,
    mode: SaveMode,
}

impl SaveRecipeStep {
    pub fn new(pool: Arc<DbPool>, user_id: Uuid) -> Self {
        Self {
            pool,
            user_id,
            mode: SaveMode::Create,
        }
    }

    pub fn for_rescrape(pool: Arc<DbPool>, user_id: Uuid, recipe_id: Uuid) -> Self {
        Self {
            pool,
            user_id,
            mode: SaveMode::Rescrape(recipe_id),
        }
    }

    pub fn for_photo_rescrape(pool: Arc<DbPool>, user_id: Uuid, recipe_id: Uuid) -> Self {
        Self {
            pool,
            user_id,
            mode: SaveMode::PhotoOnly(recipe_id),
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

        // Parse extraction method to determine version_source
        let extraction_method: Option<ExtractionMethod> = extract_output
            .get("method_used")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        // Determine version_source based on extraction method
        let version_source = match extraction_method {
            Some(ExtractionMethod::Paprika) => "import",
            Some(ExtractionMethod::PhotoUpload) => "photo_import",
            _ => match self.mode {
                SaveMode::Create => "scrape",
                SaveMode::Rescrape(_) => "rescrape",
                SaveMode::PhotoOnly(_) => "photo_rescrape",
            },
        };

        // Get photo IDs from fetch_images output
        let photo_ids: Vec<Uuid> = ctx
            .outputs
            .get_output("fetch_images")
            .and_then(|o| o.get("photo_ids").cloned())
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        // Get parsed ingredients from parse_ingredients output, or fall back to
        // simple line-by-line parsing if the step failed or is missing
        let parsed_ingredients: Vec<Ingredient> = ctx
            .outputs
            .get_output("parse_ingredients")
            .and_then(|o| o.get("ingredients").cloned())
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_else(|| {
                // Fallback: split by newlines, put each line in the item field
                raw_recipe
                    .ingredients
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(|line| Ingredient {
                        item: line.trim().to_string(),
                        measurements: vec![],
                        note: None,
                        section: None,
                    })
                    .collect()
            });

        tracing::info!(
            "save_recipe: mode={:?} user_id={} title={:?} version_source={} photos={} ingredients={}",
            self.mode,
            self.user_id,
            raw_recipe.title,
            version_source,
            photo_ids.len(),
            parsed_ingredients.len(),
        );

        // Create or update recipe in database
        let result = match self.mode {
            SaveMode::Create => {
                self.create_recipe(&raw_recipe, &photo_ids, &parsed_ingredients, version_source)
            }
            SaveMode::Rescrape(recipe_id) => self.update_recipe(
                recipe_id,
                &raw_recipe,
                &photo_ids,
                &parsed_ingredients,
                version_source,
            ),
            SaveMode::PhotoOnly(recipe_id) => {
                self.update_photos_only(recipe_id, &photo_ids, version_source)
            }
        };

        match result {
            Ok(recipe_id) => StepResult {
                step_name: SaveRecipeStepMeta::NAME.to_string(),
                success: true,
                output: json!({ "recipe_id": recipe_id.to_string() }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                // Photo-only rescrape must not run post-save enrichments:
                // they can create another version after the photo-only update.
                next_step: match self.mode {
                    SaveMode::PhotoOnly(_) => None,
                    _ => first_scrape_auto_applied_ai_step_name().map(str::to_string),
                },
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
    fn create_recipe(
        &self,
        raw: &RawRecipe,
        photo_ids: &[Uuid],
        parsed_ingredients: &[Ingredient],
        version_source: &str,
    ) -> Result<Uuid, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        let ingredients_json =
            serde_json::to_value(parsed_ingredients).map_err(|e| e.to_string())?;

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

            // 2. Create the initial version
            let new_version = NewRecipeVersion {
                recipe_id,
                title: &raw.title,
                description: raw.description.as_deref(),
                ingredients: ingredients_json.clone(),
                instructions: &raw.instructions,
                source_url: raw.source_url.as_deref(),
                source_name: raw.source_name.as_deref(),
                photo_ids: &photo_ids_nullable,
                servings: raw.servings.as_deref(),
                prep_time: raw.prep_time.as_deref(),
                cook_time: raw.cook_time.as_deref(),
                total_time: raw.total_time.as_deref(),
                rating: raw.rating,
                difficulty: raw.difficulty.as_deref(),
                nutritional_info: raw.nutritional_info.as_deref(),
                notes: raw.notes.as_deref(),
                version_source,
            };

            let version_id: Uuid = diesel::insert_into(recipe_versions::table)
                .values(&new_version)
                .returning(recipe_versions::id)
                .get_result(conn)?;

            // 3. Update recipe to point to this version
            diesel::update(recipes::table.find(recipe_id))
                .set(recipes::current_version_id.eq(version_id))
                .execute(conn)?;

            // 4. Handle categories as tags (from Paprika imports)
            if let Some(ref categories) = raw.categories {
                for tag_name in categories {
                    if tag_name.is_empty() {
                        continue;
                    }
                    let tag_id = upsert_user_tag(conn, self.user_id, tag_name)?;

                    // Insert into junction table
                    diesel::insert_into(recipe_version_tags::table)
                        .values(RecipeVersionTag {
                            recipe_version_id: version_id,
                            tag_id,
                        })
                        .on_conflict_do_nothing()
                        .execute(conn)?;
                }
            }

            Ok(recipe_id)
        })
        .map_err(|e: diesel::result::Error| e.to_string())
    }

    fn update_recipe(
        &self,
        recipe_id: Uuid,
        raw: &RawRecipe,
        photo_ids: &[Uuid],
        parsed_ingredients: &[Ingredient],
        version_source: &str,
    ) -> Result<Uuid, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        let ingredients_json =
            serde_json::to_value(parsed_ingredients).map_err(|e| e.to_string())?;

        // Convert photo IDs to Option<Uuid> for the database
        let photo_ids_nullable: Vec<Option<Uuid>> = photo_ids.iter().map(|id| Some(*id)).collect();

        // Use a transaction to create new version and update recipe
        conn.transaction(|conn| {
            // Create a new version
            let new_version = NewRecipeVersion {
                recipe_id,
                title: &raw.title,
                description: raw.description.as_deref(),
                ingredients: ingredients_json.clone(),
                instructions: &raw.instructions,
                source_url: raw.source_url.as_deref(),
                source_name: raw.source_name.as_deref(),
                photo_ids: &photo_ids_nullable,
                servings: raw.servings.as_deref(),
                prep_time: raw.prep_time.as_deref(),
                cook_time: raw.cook_time.as_deref(),
                total_time: raw.total_time.as_deref(),
                rating: raw.rating,
                difficulty: raw.difficulty.as_deref(),
                nutritional_info: raw.nutritional_info.as_deref(),
                notes: None,
                version_source,
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

    /// Create a new version that copies every field from the recipe's current
    /// version and only replaces `photo_ids`. Used by photo-only rescrape to
    /// refresh the image without losing any other edits.
    ///
    /// If no new photos were fetched, we refuse to write a new version — the
    /// image-fetch step is tolerant of failures (bad URL, CDN timeout, no
    /// image in the extracted recipe) and we would rather the job fail loudly
    /// than silently drop the recipe's existing photos.
    fn update_photos_only(
        &self,
        recipe_id: Uuid,
        photo_ids: &[Uuid],
        version_source: &str,
    ) -> Result<Uuid, String> {
        use crate::models::{Recipe, RecipeVersion, RecipeVersionTag};

        if photo_ids.is_empty() {
            return Err(
                "Photo rescrape fetched no new images; keeping existing photos".to_string(),
            );
        }

        let mut conn = self.pool.get().map_err(|e| e.to_string())?;
        let photo_ids_nullable: Vec<Option<Uuid>> = photo_ids.iter().map(|id| Some(*id)).collect();

        conn.transaction(|conn| {
            let recipe: Recipe = recipes::table.find(recipe_id).first(conn)?;
            let current_version_id = recipe
                .current_version_id
                .ok_or_else(|| diesel::result::Error::RollbackTransaction)?;
            let current: RecipeVersion = recipe_versions::table
                .find(current_version_id)
                .first(conn)?;

            let existing_tag_ids: Vec<Uuid> = recipe_version_tags::table
                .filter(recipe_version_tags::recipe_version_id.eq(current_version_id))
                .select(recipe_version_tags::tag_id)
                .load(conn)?;

            let new_version = NewRecipeVersion {
                recipe_id,
                title: &current.title,
                description: current.description.as_deref(),
                ingredients: current.ingredients.clone(),
                instructions: &current.instructions,
                source_url: current.source_url.as_deref(),
                source_name: current.source_name.as_deref(),
                photo_ids: &photo_ids_nullable,
                servings: current.servings.as_deref(),
                prep_time: current.prep_time.as_deref(),
                cook_time: current.cook_time.as_deref(),
                total_time: current.total_time.as_deref(),
                rating: current.rating,
                difficulty: current.difficulty.as_deref(),
                nutritional_info: current.nutritional_info.as_deref(),
                notes: current.notes.as_deref(),
                version_source,
            };

            let version_id: Uuid = diesel::insert_into(recipe_versions::table)
                .values(&new_version)
                .returning(recipe_versions::id)
                .get_result(conn)?;

            diesel::update(recipes::table.find(recipe_id))
                .set(recipes::current_version_id.eq(version_id))
                .execute(conn)?;

            for tag_id in &existing_tag_ids {
                diesel::insert_into(recipe_version_tags::table)
                    .values(RecipeVersionTag {
                        recipe_version_id: version_id,
                        tag_id: *tag_id,
                    })
                    .on_conflict_do_nothing()
                    .execute(conn)?;
            }

            Ok(recipe_id)
        })
        .map_err(|e: diesel::result::Error| e.to_string())
    }
}

fn copy_recipe_version_tags(
    conn: &mut diesel::PgConnection,
    old_version_id: Uuid,
    new_version_id: Uuid,
) -> Result<(), diesel::result::Error> {
    let existing_tag_ids: Vec<Uuid> = recipe_version_tags::table
        .filter(recipe_version_tags::recipe_version_id.eq(old_version_id))
        .select(recipe_version_tags::tag_id)
        .load(conn)?;

    for tag_id in existing_tag_ids {
        diesel::insert_into(recipe_version_tags::table)
            .values(RecipeVersionTag {
                recipe_version_id: new_version_id,
                tag_id,
            })
            .on_conflict_do_nothing()
            .execute(conn)?;
    }

    Ok(())
}

/// Server implementation of ApplyNormalizedTitle step.
pub struct ApplyNormalizedTitleStep {
    pool: Arc<DbPool>,
}

impl ApplyNormalizedTitleStep {
    pub const NAME: &'static str = "apply_normalized_title";

    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PipelineStep for ApplyNormalizedTitleStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Apply normalized recipe title",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        let recipe_id = match recipe_id_from_save_output(ctx) {
            Ok(id) => id,
            Err(result) => return result.with_step(Self::NAME, start, Self::NAME),
        };

        let normalize_output = ctx.outputs.get_output("enrich_normalize_title");
        let changed = normalize_output
            .as_ref()
            .and_then(|o| o.get("changed"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let normalized_title = normalize_output
            .as_ref()
            .and_then(|o| o.get("normalized_title"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if !changed || normalized_title.is_empty() {
            return StepResult {
                step_name: Self::NAME.to_string(),
                success: true,
                output: json!({ "changed": false, "message": "No title change to apply" }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: step_after_scrape_auto_applied_ai_step(Self::NAME).map(str::to_string),
            };
        }

        match self.apply_title(recipe_id, &normalized_title) {
            Ok(version_id) => StepResult {
                step_name: Self::NAME.to_string(),
                success: true,
                output: json!({
                    "changed": true,
                    "normalized_title": normalized_title,
                    "new_version_id": version_id.to_string(),
                }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: step_after_scrape_auto_applied_ai_step(Self::NAME).map(str::to_string),
            },
            Err(e) => StepResult {
                step_name: Self::NAME.to_string(),
                success: false,
                output: json!({ "error": e }),
                error: Some(e),
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: step_after_scrape_auto_applied_ai_step(Self::NAME).map(str::to_string),
            },
        }
    }
}

impl ApplyNormalizedTitleStep {
    fn apply_title(&self, recipe_id: Uuid, normalized_title: &str) -> Result<Uuid, String> {
        use crate::models::{Recipe, RecipeVersion};

        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        conn.transaction(|conn| {
            let recipe: Recipe = recipes::table.find(recipe_id).first(conn)?;
            let current_version_id = recipe
                .current_version_id
                .ok_or_else(|| diesel::result::Error::RollbackTransaction)?;
            let current: RecipeVersion = recipe_versions::table
                .find(current_version_id)
                .first(conn)?;

            if current.title == normalized_title {
                return Ok(current_version_id);
            }

            let new_version = NewRecipeVersion {
                recipe_id,
                title: normalized_title,
                description: current.description.as_deref(),
                ingredients: current.ingredients.clone(),
                instructions: &current.instructions,
                source_url: current.source_url.as_deref(),
                source_name: current.source_name.as_deref(),
                photo_ids: &current.photo_ids,
                servings: current.servings.as_deref(),
                prep_time: current.prep_time.as_deref(),
                cook_time: current.cook_time.as_deref(),
                total_time: current.total_time.as_deref(),
                rating: current.rating,
                difficulty: current.difficulty.as_deref(),
                nutritional_info: current.nutritional_info.as_deref(),
                notes: current.notes.as_deref(),
                version_source: "normalize_title",
            };

            let new_version_id: Uuid = diesel::insert_into(recipe_versions::table)
                .values(&new_version)
                .returning(recipe_versions::id)
                .get_result(conn)?;

            diesel::update(recipes::table.find(recipe_id))
                .set(recipes::current_version_id.eq(new_version_id))
                .execute(conn)?;

            copy_recipe_version_tags(conn, current_version_id, new_version_id)?;

            Ok(new_version_id)
        })
        .map_err(|e: diesel::result::Error| e.to_string())
    }
}

/// Server implementation of ApplyGeneratedDescription step.
pub struct ApplyGeneratedDescriptionStep {
    pool: Arc<DbPool>,
}

impl ApplyGeneratedDescriptionStep {
    pub const NAME: &'static str = "apply_generated_description";

    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PipelineStep for ApplyGeneratedDescriptionStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: Self::NAME,
            description: "Apply generated recipe description",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        let recipe_id = match recipe_id_from_save_output(ctx) {
            Ok(id) => id,
            Err(result) => return result.with_step(Self::NAME, start, Self::NAME),
        };

        let description_output = ctx.outputs.get_output("enrich_generate_description");
        let changed = description_output
            .as_ref()
            .and_then(|o| o.get("changed"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let generated_description = description_output
            .as_ref()
            .and_then(|o| o.get("generated_description"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if !changed || generated_description.is_empty() {
            return StepResult {
                step_name: Self::NAME.to_string(),
                success: true,
                output: json!({ "changed": false, "message": "No description change to apply" }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: step_after_scrape_auto_applied_ai_step(Self::NAME).map(str::to_string),
            };
        }

        match self.apply_description(recipe_id, &generated_description) {
            Ok(version_id) => StepResult {
                step_name: Self::NAME.to_string(),
                success: true,
                output: json!({
                    "changed": true,
                    "generated_description": generated_description,
                    "new_version_id": version_id.to_string(),
                }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: step_after_scrape_auto_applied_ai_step(Self::NAME).map(str::to_string),
            },
            Err(e) => StepResult {
                step_name: Self::NAME.to_string(),
                success: false,
                output: json!({ "error": e }),
                error: Some(e),
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: step_after_scrape_auto_applied_ai_step(Self::NAME).map(str::to_string),
            },
        }
    }
}

impl ApplyGeneratedDescriptionStep {
    fn apply_description(&self, recipe_id: Uuid, description: &str) -> Result<Uuid, String> {
        use crate::models::{Recipe, RecipeVersion};

        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        conn.transaction(|conn| {
            let recipe: Recipe = recipes::table.find(recipe_id).first(conn)?;
            let current_version_id = recipe
                .current_version_id
                .ok_or_else(|| diesel::result::Error::RollbackTransaction)?;
            let current: RecipeVersion = recipe_versions::table
                .find(current_version_id)
                .first(conn)?;

            if current.description.as_deref() == Some(description) {
                return Ok(current_version_id);
            }

            let new_version = NewRecipeVersion {
                recipe_id,
                title: &current.title,
                description: Some(description),
                ingredients: current.ingredients.clone(),
                instructions: &current.instructions,
                source_url: current.source_url.as_deref(),
                source_name: current.source_name.as_deref(),
                photo_ids: &current.photo_ids,
                servings: current.servings.as_deref(),
                prep_time: current.prep_time.as_deref(),
                cook_time: current.cook_time.as_deref(),
                total_time: current.total_time.as_deref(),
                rating: current.rating,
                difficulty: current.difficulty.as_deref(),
                nutritional_info: current.nutritional_info.as_deref(),
                notes: current.notes.as_deref(),
                version_source: "generate_description",
            };

            let new_version_id: Uuid = diesel::insert_into(recipe_versions::table)
                .values(&new_version)
                .returning(recipe_versions::id)
                .get_result(conn)?;

            diesel::update(recipes::table.find(recipe_id))
                .set(recipes::current_version_id.eq(new_version_id))
                .execute(conn)?;

            copy_recipe_version_tags(conn, current_version_id, new_version_id)?;

            Ok(new_version_id)
        })
        .map_err(|e: diesel::result::Error| e.to_string())
    }
}

enum SaveOutputReadError {
    MissingRecipeId,
}

trait SaveOutputReadErrorExt {
    fn with_step(self, step_name: &str, start: Instant, next_from: &str) -> StepResult;
}

impl SaveOutputReadErrorExt for SaveOutputReadError {
    fn with_step(self, step_name: &str, start: Instant, next_from: &str) -> StepResult {
        let message = match self {
            SaveOutputReadError::MissingRecipeId => "No recipe_id in save_recipe output",
        };
        StepResult {
            step_name: step_name.to_string(),
            success: false,
            output: json!({ "error": message }),
            error: Some(message.to_string()),
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: step_after_scrape_auto_applied_ai_step(next_from).map(str::to_string),
        }
    }
}

fn recipe_id_from_save_output(ctx: &StepContext<'_>) -> Result<Uuid, SaveOutputReadError> {
    ctx.outputs
        .get_output("save_recipe")
        .as_ref()
        .and_then(|o| o.get("recipe_id"))
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or(SaveOutputReadError::MissingRecipeId)
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
                    next_step: step_after_scrape_auto_applied_ai_step(Self::NAME)
                        .map(str::to_string),
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
                next_step: step_after_scrape_auto_applied_ai_step(Self::NAME).map(str::to_string),
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
                next_step: step_after_scrape_auto_applied_ai_step(Self::NAME).map(str::to_string),
            },
            Err(e) => StepResult {
                step_name: Self::NAME.to_string(),
                success: false,
                output: json!({ "error": e }),
                error: Some(e),
                duration_ms: start.elapsed().as_millis() as u64,
                next_step: step_after_scrape_auto_applied_ai_step(Self::NAME).map(str::to_string),
            },
        }
    }
}

impl ApplyAutoTagsStep {
    fn apply_tags(&self, recipe_id: Uuid, new_tags: &[String]) -> Result<Uuid, String> {
        use crate::models::{Recipe, RecipeVersion, RecipeVersionTag};

        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        // Get the recipe to find user_id and current_version_id
        let recipe: Recipe = recipes::table
            .find(recipe_id)
            .first(&mut conn)
            .map_err(|e| e.to_string())?;

        let current_version_id = recipe
            .current_version_id
            .ok_or("Recipe has no current version")?;

        // Fetch current version data
        let current_version: RecipeVersion = recipe_versions::table
            .find(current_version_id)
            .first(&mut conn)
            .map_err(|e| e.to_string())?;

        // Fetch existing tags from current version
        let existing_tag_ids: Vec<Uuid> = recipe_version_tags::table
            .filter(recipe_version_tags::recipe_version_id.eq(current_version_id))
            .select(recipe_version_tags::tag_id)
            .load(&mut conn)
            .unwrap_or_default();

        // Create new version with AI-suggested tags
        conn.transaction(|conn| {
            // 1. Create new version (copy all data, change version_source to "enrichment")
            let new_version = NewRecipeVersion {
                recipe_id,
                title: &current_version.title,
                description: current_version.description.as_deref(),
                ingredients: current_version.ingredients.clone(),
                instructions: &current_version.instructions,
                source_url: current_version.source_url.as_deref(),
                source_name: current_version.source_name.as_deref(),
                photo_ids: &current_version.photo_ids,
                servings: current_version.servings.as_deref(),
                prep_time: current_version.prep_time.as_deref(),
                cook_time: current_version.cook_time.as_deref(),
                total_time: current_version.total_time.as_deref(),
                rating: current_version.rating,
                difficulty: current_version.difficulty.as_deref(),
                nutritional_info: current_version.nutritional_info.as_deref(),
                notes: current_version.notes.as_deref(),
                version_source: "enrichment",
            };

            let new_version_id: Uuid = diesel::insert_into(recipe_versions::table)
                .values(&new_version)
                .returning(recipe_versions::id)
                .get_result(conn)?;

            // 2. Update recipe to point to new version
            diesel::update(recipes::table.find(recipe_id))
                .set(recipes::current_version_id.eq(new_version_id))
                .execute(conn)?;

            // 3. Copy existing tags to new version
            for tag_id in &existing_tag_ids {
                diesel::insert_into(recipe_version_tags::table)
                    .values(RecipeVersionTag {
                        recipe_version_id: new_version_id,
                        tag_id: *tag_id,
                    })
                    .on_conflict_do_nothing()
                    .execute(conn)?;
            }

            // 4. Add new AI-suggested tags
            for tag_name in new_tags {
                let tag_id = upsert_user_tag(conn, recipe.user_id, tag_name)?;

                // Insert into junction table (skip if already exists from copied tags)
                diesel::insert_into(recipe_version_tags::table)
                    .values(RecipeVersionTag {
                        recipe_version_id: new_version_id,
                        tag_id,
                    })
                    .on_conflict_do_nothing()
                    .execute(conn)?;
            }

            Ok(new_version_id)
        })
        .map_err(|e: diesel::result::Error| e.to_string())
    }
}

// Enrich steps use generic implementations from ramekin-core.
