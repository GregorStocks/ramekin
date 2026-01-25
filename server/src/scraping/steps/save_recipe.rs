//! Server SaveRecipe step - creates Recipe records in the database.

use std::time::Instant;

use async_trait::async_trait;
use diesel::prelude::*;
use ramekin_core::pipeline::steps::SaveRecipeStepMeta;
use ramekin_core::pipeline::{PipelineStep, StepContext, StepMetadata, StepResult};
use ramekin_core::RawRecipe;
use serde_json::json;
use uuid::Uuid;

use crate::db::DbPool;
use crate::models::{Ingredient, NewRecipe, NewRecipeVersion};
use crate::schema::{recipe_versions, recipes};

/// Server implementation of SaveRecipe - creates Recipe records in the database.
pub struct SaveRecipeStep<'a> {
    pool: &'a DbPool,
    user_id: Uuid,
}

impl<'a> SaveRecipeStep<'a> {
    /// Create a new SaveRecipeStep.
    pub fn new(pool: &'a DbPool, user_id: Uuid) -> Self {
        Self { pool, user_id }
    }

    /// Create a recipe from RawRecipe.
    fn create_recipe(&self, raw: &RawRecipe, photo_ids: &[Uuid]) -> Result<Uuid, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        // Convert raw ingredients (newline-separated blob) to our Ingredient JSON format
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
                ingredients: ingredients_json,
                instructions: &raw.instructions,
                source_url: Some(&raw.source_url),
                source_name: raw.source_name.as_deref(),
                photo_ids: &photo_ids_nullable,
                tags: &[],
                // Paprika-compatible fields - not populated from web scraping
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
}

#[async_trait]
impl PipelineStep for SaveRecipeStep<'_> {
    fn metadata(&self) -> StepMetadata {
        SaveRecipeStepMeta::metadata()
    }

    async fn execute(&self, ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();

        // Get raw_recipe from extract output
        let extract_output = match ctx.outputs.get_output("extract_recipe") {
            Some(o) => o,
            None => {
                return StepResult {
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some("extract_recipe output not found".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        let raw_recipe: RawRecipe = match extract_output.get("raw_recipe") {
            Some(v) => match serde_json::from_value(v.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return StepResult {
                        success: false,
                        output: serde_json::Value::Null,
                        error: Some(format!("Failed to parse raw_recipe: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        next_step: None,
                    };
                }
            },
            None => {
                return StepResult {
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some("No raw_recipe in extract_recipe output".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                };
            }
        };

        // Get photo IDs from fetch_images output (may not exist for old jobs)
        let photo_ids: Vec<Uuid> = ctx
            .outputs
            .get_output("fetch_images")
            .and_then(|o| o.get("photo_ids").cloned())
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        // Create the recipe
        match self.create_recipe(&raw_recipe, &photo_ids) {
            Ok(recipe_id) => {
                tracing::info!(
                    "Created recipe {} with {} photos",
                    recipe_id,
                    photo_ids.len()
                );

                StepResult {
                    success: true,
                    output: json!({ "recipe_id": recipe_id }),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: Some("enrich".to_string()),
                }
            }
            Err(e) => {
                tracing::error!("Failed to create recipe: {}", e);

                StepResult {
                    success: false,
                    output: serde_json::Value::Null,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    next_step: None,
                }
            }
        }
    }
}
