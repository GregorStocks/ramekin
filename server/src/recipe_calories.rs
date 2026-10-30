//! Each recipe version's stored per-serving calorie estimate, which the
//! recipe list sorts, filters and shows. Stored when a version is written,
//! recomputed when one of its ingredient names resolves
//! (`ingredient_names::save_resolved`), and reconciled at startup, which
//! fills in versions saved before it was stored and picks up a catalog
//! deploy's changes. Only current versions are kept up to date.

use crate::db::DbPool;
use crate::ids::Uuid;
use crate::models::Ingredient;
use crate::schema::{recipe_versions, recipes};
use diesel::prelude::*;
use ramekin_core::catalog::{normalize, Learned};
use ramekin_core::ingredient_parser::ParsedIngredient;
use ramekin_core::nutrition;
use std::collections::HashSet;

/// What the columns hold: (kcal_per_serving, kcal_per_serving_label).
type Stored = (Option<f64>, Option<String>);

/// A current version's id, servings and ingredients, with what's stored.
type Row = (
    Uuid,
    Option<String>,
    serde_json::Value,
    Option<f64>,
    Option<String>,
);

fn parsed(ingredients: &serde_json::Value) -> Vec<ParsedIngredient> {
    // Stored ingredients are written from `Ingredient`; anything that no
    // longer reads as one has no estimate rather than failing the write.
    serde_json::from_value::<Vec<Ingredient>>(ingredients.clone())
        .map(|ingredients| ingredients.into_iter().map(Into::into).collect())
        .unwrap_or_default()
}

fn figure(ingredients: &[ParsedIngredient], servings: Option<&str>, learned: &Learned) -> Stored {
    match nutrition::estimate_with(ingredients, servings, 1.0, learned)
        .ok()
        .and_then(|estimate| estimate.serving_figure())
    {
        Some(figure) => (Some(figure.kcal), Some(figure.label)),
        None => (None, None),
    }
}

/// Store the estimate of a version this write just inserted. Call inside
/// `db::write`, so the name worker can't resolve one of its names in between.
pub fn store(conn: &mut SqliteConnection, version_id: Uuid) -> QueryResult<()> {
    let (servings, ingredients): (Option<String>, serde_json::Value) = recipe_versions::table
        .find(version_id)
        .select((recipe_versions::servings, recipe_versions::ingredients))
        .first(conn)?;
    let ingredients = parsed(&ingredients);
    let learned =
        crate::ingredient_names::load_learned(conn, ingredients.iter().map(|i| i.item.as_str()))?;
    let (kcal, label) = figure(&ingredients, servings.as_deref(), &learned);
    diesel::update(recipe_versions::table.find(version_id))
        .set((
            recipe_versions::kcal_per_serving.eq(kcal),
            recipe_versions::kcal_per_serving_label.eq(label),
        ))
        .execute(conn)?;
    Ok(())
}

/// Recompute current versions whose ingredients `keep` selects, writing only
/// those whose estimate changed. Returns how many changed.
fn recompute(
    conn: &mut SqliteConnection,
    keep: impl Fn(&[ParsedIngredient]) -> bool,
) -> QueryResult<usize> {
    let rows: Vec<Row> = recipes::table
        .inner_join(
            recipe_versions::table.on(recipe_versions::id
                .nullable()
                .eq(recipes::current_version_id)),
        )
        .filter(recipes::deleted_at.is_null())
        .select((
            recipe_versions::id,
            recipe_versions::servings,
            recipe_versions::ingredients,
            recipe_versions::kcal_per_serving,
            recipe_versions::kcal_per_serving_label,
        ))
        .load(conn)?;
    let rows: Vec<(Uuid, Option<String>, Vec<ParsedIngredient>, Stored)> = rows
        .into_iter()
        .map(|(id, servings, ingredients, kcal, label)| {
            (id, servings, parsed(&ingredients), (kcal, label))
        })
        .filter(|(_, _, ingredients, _)| keep(ingredients))
        .collect();
    let learned = crate::ingredient_names::load_learned(
        conn,
        rows.iter()
            .flat_map(|(_, _, ingredients, _)| ingredients.iter().map(|i| i.item.as_str())),
    )?;
    let mut changed = 0;
    for (id, servings, ingredients, stored) in &rows {
        let (kcal, label) = figure(ingredients, servings.as_deref(), &learned);
        if (&kcal, &label) != (&stored.0, &stored.1) {
            diesel::update(recipe_versions::table.find(id))
                .set((
                    recipe_versions::kcal_per_serving.eq(kcal),
                    recipe_versions::kcal_per_serving_label.eq(label),
                ))
                .execute(conn)?;
            changed += 1;
        }
    }
    Ok(changed)
}

/// Recompute current versions using any of these names, in the write that
/// stores their new answers. Names match by catalog normalization, which no
/// SQL pattern expresses, so current versions are read and matched here.
pub fn recompute_for_names(
    conn: &mut SqliteConnection,
    changed: &HashSet<String>,
) -> QueryResult<usize> {
    if changed.is_empty() {
        return Ok(0);
    }
    recompute(conn, |ingredients| {
        ingredients
            .iter()
            .any(|i| changed.contains(normalize(&i.item).as_str()))
    })
}

/// Bring every current version's stored estimate up to date. Runs at startup,
/// after learned names are synced with the catalog and before the
/// ingredient-name worker starts; returns how many versions changed.
pub fn reconcile(pool: &DbPool) -> Result<usize, String> {
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    crate::db::write(&mut conn, |conn| recompute(conn, |_| true))
        .map_err(|e: diesel::result::Error| e.to_string())
}
