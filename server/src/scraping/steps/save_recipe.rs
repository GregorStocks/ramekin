use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use diesel::prelude::*;
use serde_json::json;
use uuid::Uuid;

use ramekin_core::pipeline::{
    deserialize_optional_output_field, deserialize_required_output_field,
    first_scrape_auto_applied_ai_step_name, steps::SaveRecipeStepMeta, PipelineStep, StepContext,
    StepMetadata, StepResult,
};
use ramekin_core::{ExtractionMethod, RawRecipe};

use crate::db::{run_blocking, DbPool};
use crate::models::{Ingredient, NewRecipeVersion};
use crate::recipes::{create_new_version_cas, insert_recipe, TagSource, VersionWriteError};
use crate::schema::recipe_versions;

/// How SaveRecipeStep should behave.
#[derive(Debug, Clone, Copy)]
pub enum SaveMode {
    /// Create a brand-new recipe.
    Create,
    /// Update an existing recipe by creating a new version from the newly
    /// scraped data.
    Rescrape {
        recipe_id: Uuid,
        expected_version_id: Uuid,
    },
    /// Update an existing recipe by creating a new version that copies every
    /// field from the current version and only replaces `photo_ids` with the
    /// newly-fetched photos.
    PhotoOnly {
        recipe_id: Uuid,
        expected_version_id: Uuid,
    },
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

    pub fn for_rescrape(
        pool: Arc<DbPool>,
        user_id: Uuid,
        recipe_id: Uuid,
        expected_version_id: Uuid,
    ) -> Self {
        Self {
            pool,
            user_id,
            mode: SaveMode::Rescrape {
                recipe_id,
                expected_version_id,
            },
        }
    }

    pub fn for_photo_rescrape(
        pool: Arc<DbPool>,
        user_id: Uuid,
        recipe_id: Uuid,
        expected_version_id: Uuid,
    ) -> Self {
        Self {
            pool,
            user_id,
            mode: SaveMode::PhotoOnly {
                recipe_id,
                expected_version_id,
            },
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
        let extract_output = match ctx.outputs.get_output("extract_recipe").await {
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
        let raw_recipe: RawRecipe = match deserialize_required_output_field(
            &extract_output,
            "extract_recipe",
            "raw_recipe",
        ) {
            Ok(r) => r,
            Err(e) => {
                return StepResult {
                    step_name: SaveRecipeStepMeta::NAME.to_string(),
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Parse extraction method to determine version_source
        let extraction_method: Option<ExtractionMethod> = match deserialize_optional_output_field(
            Some(&extract_output),
            "extract_recipe",
            "method_used",
        ) {
            Ok(method) => method,
            Err(e) => {
                return StepResult {
                    step_name: SaveRecipeStepMeta::NAME.to_string(),
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Determine version_source based on extraction method
        let version_source = match extraction_method {
            Some(ExtractionMethod::Paprika) => "import",
            Some(ExtractionMethod::PhotoUpload) => "photo_import",
            _ => match self.mode {
                SaveMode::Create => "scrape",
                SaveMode::Rescrape { .. } => "rescrape",
                SaveMode::PhotoOnly { .. } => "photo_rescrape",
            },
        };

        // Get photo IDs from fetch_images output
        let fetch_images_output = ctx.outputs.get_output("fetch_images").await;
        let photo_ids: Vec<Uuid> = match deserialize_optional_output_field(
            fetch_images_output.as_ref(),
            "fetch_images",
            "photo_ids",
        ) {
            Ok(Some(ids)) => ids,
            Ok(None) => Vec::new(),
            Err(e) => {
                return StepResult {
                    step_name: SaveRecipeStepMeta::NAME.to_string(),
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Get parsed ingredients from parse_ingredients output, or fall back to
        // simple line-by-line parsing if the step failed or is missing
        let parse_ingredients_output = ctx.outputs.get_output("parse_ingredients").await;
        let parsed_ingredients: Vec<Ingredient> = match deserialize_optional_output_field(
            parse_ingredients_output.as_ref(),
            "parse_ingredients",
            "ingredients",
        ) {
            Ok(Some(ingredients)) => ingredients,
            Ok(None) => {
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
            }
            Err(e) => {
                return StepResult {
                    step_name: SaveRecipeStepMeta::NAME.to_string(),
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

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
                self.create_recipe(raw_recipe, &photo_ids, &parsed_ingredients, version_source)
                    .await
            }
            SaveMode::Rescrape {
                recipe_id,
                expected_version_id,
            } => {
                self.update_recipe(
                    recipe_id,
                    expected_version_id,
                    raw_recipe,
                    &photo_ids,
                    &parsed_ingredients,
                    version_source,
                )
                .await
            }
            SaveMode::PhotoOnly {
                recipe_id,
                expected_version_id,
            } => {
                self.update_photos_only(recipe_id, expected_version_id, &photo_ids, version_source)
                    .await
            }
        };

        match result {
            Ok((recipe_id, version_id)) => StepResult {
                step_name: SaveRecipeStepMeta::NAME.to_string(),
                success: true,
                output: json!({
                    "recipe_id": recipe_id.to_string(),
                    "version_id": version_id.to_string(),
                }),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                // Photo-only rescrape must not run post-save enrichments:
                // they can create another version after the photo-only update.
                next_step: match self.mode {
                    SaveMode::PhotoOnly { .. } => None,
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
    async fn create_recipe(
        &self,
        raw: RawRecipe,
        photo_ids: &[Uuid],
        parsed_ingredients: &[Ingredient],
        version_source: &str,
    ) -> Result<(Uuid, Uuid), String> {
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

        let version_source = version_source.to_string();
        let user_id = self.user_id;

        // Use a transaction to create recipe + version atomically
        run_blocking(&self.pool, move |conn| {
            conn.transaction(|conn| {
                let recipe_id = insert_recipe(conn, user_id)?;

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
                    version_source: &version_source,
                };

                let version_id = create_new_version_cas(
                    conn,
                    &new_version,
                    None,
                    TagSource::Names {
                        user_id,
                        names: &category_tags,
                    },
                )?;

                Ok((recipe_id, version_id))
            })
            .map_err(|e: VersionWriteError| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?
    }

    async fn update_recipe(
        &self,
        recipe_id: Uuid,
        expected_version_id: Uuid,
        raw: RawRecipe,
        photo_ids: &[Uuid],
        parsed_ingredients: &[Ingredient],
        version_source: &str,
    ) -> Result<(Uuid, Uuid), String> {
        let ingredients_json =
            serde_json::to_value(parsed_ingredients).map_err(|e| e.to_string())?;

        // Convert photo IDs to Option<Uuid> for the database
        let photo_ids_nullable: Vec<Option<Uuid>> = photo_ids.iter().map(|id| Some(*id)).collect();

        let version_source = version_source.to_string();

        // Use a transaction to create a new version only if the recipe has not
        // changed since this rescrape job was created.
        run_blocking(&self.pool, move |conn| {
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
                    version_source: &version_source,
                };

                let version_id = create_new_version_cas(
                    conn,
                    &new_version,
                    Some(expected_version_id),
                    TagSource::CopyFrom(expected_version_id),
                )?;

                Ok((recipe_id, version_id))
            })
            .map_err(|e: VersionWriteError| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// Create a new version that copies every field from the recipe's current
    /// version and only replaces `photo_ids`. Used by photo-only rescrape to
    /// refresh the image without losing any other edits.
    ///
    /// If no new photos were fetched, we refuse to write a new version — the
    /// image-fetch step is tolerant of failures (bad URL, CDN timeout, no
    /// image in the extracted recipe) and we would rather the job fail loudly
    /// than silently drop the recipe's existing photos.
    async fn update_photos_only(
        &self,
        recipe_id: Uuid,
        expected_version_id: Uuid,
        photo_ids: &[Uuid],
        version_source: &str,
    ) -> Result<(Uuid, Uuid), String> {
        use crate::models::RecipeVersion;

        if photo_ids.is_empty() {
            return Err(
                "Photo rescrape fetched no new images; keeping existing photos".to_string(),
            );
        }

        let photo_ids_nullable: Vec<Option<Uuid>> = photo_ids.iter().map(|id| Some(*id)).collect();
        let version_source = version_source.to_string();

        run_blocking(&self.pool, move |conn| {
            conn.transaction(|conn| {
                let current: RecipeVersion = recipe_versions::table
                    .filter(recipe_versions::id.eq(expected_version_id))
                    .filter(recipe_versions::recipe_id.eq(recipe_id))
                    .select(RecipeVersion::as_select())
                    .first(conn)?;

                let new_version = NewRecipeVersion {
                    photo_ids: &photo_ids_nullable,
                    ..NewRecipeVersion::copy_of(&current, &version_source)
                };

                let version_id = create_new_version_cas(
                    conn,
                    &new_version,
                    Some(expected_version_id),
                    TagSource::CopyFrom(expected_version_id),
                )?;

                Ok((recipe_id, version_id))
            })
            .map_err(|e: VersionWriteError| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?
    }
}
