use crate::api::{run_db, run_db_locked, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::ids::Uuid;
use crate::models::{Ingredient, NewRecipeVersion, RecipeVersion};
use crate::recipes::{create_new_version_cas, TagSource, VersionWriteError};
use crate::schema::{recipe_versions, recipes, scrape_jobs, step_outputs};
use crate::scraping::output_store::latest_output;
use axum::{extract::State, Json};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use ramekin_core::pipeline::steps::parse_raw_ingredients;
use ramekin_core::types::{ExtractionMethod, RawRecipe};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Default, Serialize, ToSchema)]
pub struct ReparseIngredientsResponse {
    /// Live recipes looked at.
    pub recipes_checked: usize,
    /// Recipes that got a new "reparse" version because their ingredients
    /// changed.
    pub recipes_updated: usize,
    /// Recipes with no saved scrape to re-read: typed in, or from text or a
    /// photo, or a scrape whose page or text can't be read.
    pub skipped_no_source: usize,
    /// Recipes edited by hand since their last scrape, or changed while the
    /// re-parse ran.
    pub skipped_edited: usize,
}

/// The scrape that last saved a recipe's ingredients.
struct SavedScrape {
    job_id: Uuid,
    url: Option<String>,
    saved_at: DateTime<Utc>,
}

/// A successful `save_recipe` run: its job's recipe, the job, its URL, and
/// when it saved.
type SaveRow = (Option<Uuid>, Uuid, Option<String>, DateTime<Utc>);

/// What `extract_recipe` saved: the recipe it read and how. Text recipes
/// record no method.
#[derive(Deserialize)]
struct ExtractOutput {
    raw_recipe: RawRecipe,
    method_used: Option<ExtractionMethod>,
}

/// Each live recipe's current version, the scrape that last saved its
/// ingredients (photo-only rescrapes save none), and whether it was edited by
/// hand after that scrape.
fn load_targets(
    conn: &mut crate::db::DbConn,
    user_id: Uuid,
) -> Result<Vec<(RecipeVersion, Option<SavedScrape>, bool)>, ApiError> {
    let failed = |e: diesel::result::Error| {
        tracing::error!("Failed to load recipes for re-parse: {}", e);
        ApiError::internal("Failed to load recipes")
    };
    let versions: Vec<RecipeVersion> = recipes::table
        .inner_join(
            recipe_versions::table
                .on(recipes::current_version_id.eq(recipe_versions::id.nullable())),
        )
        .filter(recipes::user_id.eq(user_id))
        .filter(recipes::deleted_at.is_null())
        .select(RecipeVersion::as_select())
        .load(conn)
        .map_err(failed)?;

    let saves: Vec<SaveRow> = scrape_jobs::table
        .inner_join(step_outputs::table.on(step_outputs::scrape_job_id.eq(scrape_jobs::id)))
        .filter(scrape_jobs::user_id.eq(user_id))
        .filter(scrape_jobs::photo_only.eq(false))
        .filter(step_outputs::step_name.eq("save_recipe"))
        .filter(step_outputs::success.eq(true))
        .select((
            scrape_jobs::recipe_id,
            scrape_jobs::id,
            scrape_jobs::url,
            step_outputs::created_at,
        ))
        .load(conn)
        .map_err(failed)?;
    let mut latest: HashMap<Uuid, SavedScrape> = HashMap::new();
    for (recipe_id, job_id, url, saved_at) in saves {
        let Some(recipe_id) = recipe_id else { continue };
        if latest
            .get(&recipe_id)
            .is_none_or(|known| known.saved_at < saved_at)
        {
            latest.insert(
                recipe_id,
                SavedScrape {
                    job_id,
                    url,
                    saved_at,
                },
            );
        }
    }

    let edits: Vec<(Uuid, DateTime<Utc>)> = recipe_versions::table
        .inner_join(recipes::table.on(recipes::id.eq(recipe_versions::recipe_id)))
        .filter(recipes::user_id.eq(user_id))
        .filter(recipe_versions::version_source.eq("user"))
        .select((recipe_versions::recipe_id, recipe_versions::created_at))
        .load(conn)
        .map_err(failed)?;
    let mut last_edit: HashMap<Uuid, DateTime<Utc>> = HashMap::new();
    for (recipe_id, at) in edits {
        let entry = last_edit.entry(recipe_id).or_insert(at);
        *entry = (*entry).max(at);
    }

    Ok(versions
        .into_iter()
        .map(|version| {
            let scrape = latest.remove(&version.recipe_id);
            let edited = scrape.as_ref().is_some_and(|scrape| {
                last_edit
                    .get(&version.recipe_id)
                    .is_some_and(|edit| *edit > scrape.saved_at)
            });
            (version, scrape, edited)
        })
        .collect())
}

/// The ingredients the scrape would store today: its saved page extracted
/// again, so extractor fixes apply too, or, for Paprika and text imports,
/// the recipe it read. None if neither can be read.
async fn reparsed_ingredients(pool: &DbPool, scrape: &SavedScrape) -> Option<Vec<Ingredient>> {
    let html = latest_output(pool, scrape.job_id, "fetch_html", true)
        .await
        .ok()
        .flatten()
        .and_then(|output| output.get("html")?.as_str().map(str::to_string));
    let extracted = match (html, scrape.url.clone()) {
        (Some(html), Some(url)) => tokio::task::spawn_blocking(move || {
            ramekin_core::extract_recipe_with_stats(&html, &url)
                .ok()
                .map(|output| ExtractOutput {
                    raw_recipe: output.raw_recipe,
                    method_used: Some(output.method_used),
                })
        })
        .await
        .ok()
        .flatten(),
        _ => latest_output(pool, scrape.job_id, "extract_recipe", true)
            .await
            .ok()
            .flatten()
            .and_then(|output| serde_json::from_value(output).ok()),
    }?;
    let parsed = parse_raw_ingredients(&extracted.raw_recipe, extracted.method_used);
    serde_json::to_value(parsed)
        .and_then(serde_json::from_value)
        .ok()
}

#[utoipa::path(
    post,
    path = "/api/recipes/reparse-ingredients",
    tag = "recipes",
    responses(
        (status = 200, description = "Every recipe checked; changed recipes got a new version", body = ReparseIngredientsResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
/// Rebuild the ingredients of each of the user's scraped or imported recipes
/// from the scrape that saved them, with the current extractor and parser,
/// saving a new version (source "reparse") for each recipe whose ingredients
/// change. Recipes edited by hand since that scrape are left alone.
pub async fn reparse_all_ingredients(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
) -> Result<Json<ReparseIngredientsResponse>, ApiError> {
    let user_id = user.id;
    let targets = run_db(&pool, move |conn| load_targets(conn, user_id)).await?;
    let mut response = ReparseIngredientsResponse {
        recipes_checked: targets.len(),
        ..Default::default()
    };
    let result = async {
        for (current, scrape, edited) in targets {
            let Some(scrape) = scrape else {
                response.skipped_no_source += 1;
                continue;
            };
            if edited {
                response.skipped_edited += 1;
                continue;
            }
            let Some(ingredients) = reparsed_ingredients(&pool, &scrape).await else {
                response.skipped_no_source += 1;
                continue;
            };
            let ingredients_json = serde_json::to_value(&ingredients)
                .map_err(|_| ApiError::internal("Failed to serialize ingredients"))?;
            if ingredients_json == current.ingredients {
                continue;
            }
            let written = run_db_locked(&pool, move |conn| {
                let result: Result<(), VersionWriteError> = crate::db::write(conn, |conn| {
                    create_new_version_cas(
                        conn,
                        &NewRecipeVersion {
                            ingredients: ingredients_json,
                            ..NewRecipeVersion::copy_of(&current, "reparse")
                        },
                        Some(current.id),
                        TagSource::CopyFrom(current.id),
                    )?;
                    crate::ingredient_names::enqueue_items(
                        conn,
                        ingredients.iter().map(|i| i.item.as_str()),
                    )?;
                    Ok(())
                });
                match result {
                    Ok(()) => Ok(true),
                    // Changed since it was read: rolled back, and left alone.
                    Err(VersionWriteError::Stale) => Ok(false),
                    Err(VersionWriteError::Db(e)) => {
                        tracing::error!("Failed to save re-parsed recipe: {}", e);
                        Err(ApiError::internal("Failed to save re-parsed recipe"))
                    }
                }
            })
            .await?;
            if written {
                response.recipes_updated += 1;
            } else {
                response.skipped_edited += 1;
            }
        }
        Ok::<_, ApiError>(())
    }
    .await;
    // Each recipe commits on its own, so wake the worker even when a later
    // one failed: the earlier ones' names are already queued.
    crate::ingredient_names::wake();
    result?;
    tracing::info!(
        user_id = %user_id,
        recipes_checked = response.recipes_checked,
        recipes_updated = response.recipes_updated,
        skipped_no_source = response.skipped_no_source,
        skipped_edited = response.skipped_edited,
        "re-parsed stored ingredients"
    );
    Ok(Json(response))
}
