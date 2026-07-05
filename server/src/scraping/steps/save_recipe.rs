use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use diesel::prelude::*;
use serde_json::json;
use uuid::Uuid;

use ramekin_core::pipeline::{
    first_scrape_auto_applied_ai_step_name, steps::SaveRecipeStepMeta, PipelineStep, StepContext,
    StepMetadata, StepResult,
};
use ramekin_core::{ExtractionMethod, RawRecipe};

use crate::db::DbPool;
use crate::models::{Ingredient, NewRecipeVersion};
use crate::recipes::{create_new_version, insert_recipe, TagSource};
use crate::schema::{recipe_versions, recipes};

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

        // Categories come from Paprika imports and are applied as tags.
        let category_tags: Vec<String> = raw
            .categories
            .iter()
            .flatten()
            .filter(|name| !name.is_empty())
            .cloned()
            .collect();

        // Use a transaction to create recipe + version atomically
        conn.transaction(|conn| {
            let recipe_id = insert_recipe(conn, self.user_id)?;

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

            create_new_version(
                conn,
                &new_version,
                TagSource::Names {
                    user_id: self.user_id,
                    names: &category_tags,
                },
            )?;

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
            let current_version_id: Option<Uuid> = recipes::table
                .find(recipe_id)
                .select(recipes::current_version_id)
                .first(conn)?;
            let current_version_id =
                current_version_id.ok_or_else(|| diesel::result::Error::RollbackTransaction)?;

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

            create_new_version(conn, &new_version, TagSource::CopyFrom(current_version_id))?;

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
        use crate::models::{Recipe, RecipeVersion};

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

            let new_version = NewRecipeVersion {
                photo_ids: &photo_ids_nullable,
                ..NewRecipeVersion::copy_of(&current, version_source)
            };

            create_new_version(conn, &new_version, TagSource::CopyFrom(current_version_id))?;

            Ok(recipe_id)
        })
        .map_err(|e: diesel::result::Error| e.to_string())
    }
}
