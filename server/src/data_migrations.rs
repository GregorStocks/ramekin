//! Data migrations written in Rust, for changes SQL can't express without
//! duplicating server logic (anything that needs the ingredient parser or
//! catalog). Each runs once, at startup after the SQL migrations, in its own
//! transaction together with the `data_migrations` row that records it, and a
//! failure stops startup.
//!
//! Migrations use the current models and Diesel DSL, so one that has run in
//! production can be deleted once it no longer compiles; its row stays.

use crate::models::{Ingredient, NewRecipeVersion, RecipeVersion};
use crate::recipes::{create_new_version_cas, TagSource};
use crate::schema::{data_migrations, recipe_versions, recipes};
use anyhow::{anyhow, Context};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use ramekin_core::catalog::is_volume_unit;
use std::collections::HashSet;

type DataMigration = fn(&mut PgConnection) -> anyhow::Result<()>;

/// Every data migration, in the order they run. Never rename one that has run.
const DATA_MIGRATIONS: &[(&str, DataMigration)] =
    &[("strip_materialized_grams", strip_materialized_grams)];

/// Run each data migration that hasn't run yet.
pub fn run_pending(conn: &mut PgConnection) -> anyhow::Result<()> {
    let done: HashSet<String> = data_migrations::table
        .select(data_migrations::name)
        .load(conn)
        .context("failed to load completed data migrations")?
        .into_iter()
        .collect();
    for (name, migrate) in DATA_MIGRATIONS {
        if done.contains(*name) {
            continue;
        }
        conn.transaction(|conn| {
            migrate(conn)?;
            diesel::insert_into(data_migrations::table)
                .values(data_migrations::name.eq(name))
                .execute(conn)?;
            anyhow::Ok(())
        })
        .with_context(|| format!("data migration {name} failed"))?;
        tracing::info!(name, "ran data migration");
    }
    Ok(())
}

/// What stripping stored grams did to one recipe's ingredients.
struct Stripped {
    ingredients: Vec<Ingredient>,
    /// Trailing gram amounts removed because the catalog computes them.
    removed: usize,
    /// Trailing gram amounts after a volume or oz/lb that the catalog doesn't
    /// compute: given by the source, or computed with an older density.
    kept: usize,
}

/// Remove each trailing gram amount that `derived_grams` reproduces exactly
/// from the line's other measurements; it was stored by the old ingest-time
/// enrichment and is now computed on read. A source's own grams are removed
/// only when the catalog computes the same amount, so the display is
/// unchanged.
fn strip_ingredients(ingredients: Vec<Ingredient>) -> Stripped {
    let mut removed = 0;
    let mut kept = 0;
    let ingredients = ingredients
        .into_iter()
        .map(|ingredient| {
            let Some((last, rest)) = ingredient.measurements.split_last() else {
                return ingredient;
            };
            if rest.is_empty() || last.unit.as_deref() != Some("g") {
                return ingredient;
            }
            let without = Ingredient {
                measurements: rest.to_vec(),
                ..ingredient.clone()
            };
            let derived = ramekin_core::derived_grams(&without.clone().into());
            if derived.is_some_and(|derived| derived.amount == last.amount) {
                removed += 1;
                return without;
            }
            if rest.iter().any(|m| {
                is_volume_unit(m.unit.as_deref()) || matches!(m.unit.as_deref(), Some("oz" | "lb"))
            }) {
                kept += 1;
            }
            ingredient
        })
        .collect();
    Stripped {
        ingredients,
        removed,
        kept,
    }
}

/// Gram amounts were stored at ingest; they're now computed when a recipe is
/// read. Every live recipe whose current version has stored grams the catalog
/// reproduces gets a new version (source "migration") without them. Earlier
/// versions are kept.
fn strip_materialized_grams(conn: &mut PgConnection) -> anyhow::Result<()> {
    let versions: Vec<RecipeVersion> = recipes::table
        .inner_join(
            recipe_versions::table
                .on(recipes::current_version_id.eq(recipe_versions::id.nullable())),
        )
        .filter(recipes::deleted_at.is_null())
        .select(RecipeVersion::as_select())
        .load(conn)?;

    let (mut recipes_updated, mut removed, mut kept) = (0, 0, 0);
    for current in &versions {
        let ingredients: Vec<Ingredient> = serde_json::from_value(current.ingredients.clone())
            .with_context(|| format!("recipe version {} has invalid ingredients", current.id))?;
        let stripped = strip_ingredients(ingredients);
        kept += stripped.kept;
        if stripped.removed == 0 {
            continue;
        }
        create_new_version_cas(
            conn,
            &NewRecipeVersion {
                ingredients: serde_json::to_value(&stripped.ingredients)?,
                ..NewRecipeVersion::copy_of(current, "migration")
            },
            Some(current.id),
            TagSource::CopyFrom(current.id),
        )
        .map_err(|e| anyhow!("failed to save recipe {}: {e}", current.recipe_id))?;
        recipes_updated += 1;
        removed += stripped.removed;
    }
    tracing::info!(
        recipes_checked = versions.len(),
        recipes_updated,
        grams_removed = removed,
        grams_kept = kept,
        "stripped stored gram amounts the catalog computes"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Measurement;

    fn ingredient(item: &str, measurements: &[(&str, &str)]) -> Ingredient {
        Ingredient {
            item: item.to_string(),
            measurements: measurements
                .iter()
                .map(|(amount, unit)| Measurement {
                    amount: Some(amount.to_string()),
                    unit: Some(unit.to_string()),
                })
                .collect(),
            note: None,
            section: None,
        }
    }

    fn units(stripped: &Stripped) -> Vec<Vec<String>> {
        stripped
            .ingredients
            .iter()
            .map(|i| {
                i.measurements
                    .iter()
                    .map(|m| {
                        format!(
                            "{} {}",
                            m.amount.as_deref().unwrap_or(""),
                            m.unit.as_deref().unwrap_or("")
                        )
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn strips_grams_the_catalog_computes() {
        let stripped = strip_ingredients(vec![
            ingredient("all-purpose flour", &[("2", "cup"), ("250", "g")]),
            ingredient("butter", &[("8", "oz"), ("227", "g")]),
        ]);
        assert_eq!(units(&stripped), vec![vec!["2 cup"], vec!["8 oz"]]);
        assert_eq!((stripped.removed, stripped.kept), (2, 0));
    }

    #[test]
    fn keeps_grams_the_catalog_does_not_compute() {
        let stripped = strip_ingredients(vec![
            // A source's own amount, or an older density.
            ingredient("all-purpose flour", &[("1", "cup"), ("120", "g")]),
            ingredient("unicorn tears", &[("1", "cup"), ("200", "g")]),
        ]);
        assert_eq!(
            units(&stripped),
            vec![vec!["1 cup", "120 g"], vec!["1 cup", "200 g"]]
        );
        assert_eq!((stripped.removed, stripped.kept), (0, 2));
    }

    #[test]
    fn leaves_lines_without_a_trailing_gram_amount() {
        let stripped = strip_ingredients(vec![
            ingredient("flour", &[("250", "g")]),
            ingredient("flour", &[("250", "g"), ("2", "cup")]),
            ingredient("eggs", &[("2", "")]),
            ingredient("salt", &[]),
        ]);
        assert_eq!(
            units(&stripped),
            vec![vec!["250 g"], vec!["250 g", "2 cup"], vec!["2 "], vec![]]
        );
        assert_eq!((stripped.removed, stripped.kept), (0, 0));
    }

    #[test]
    fn keeps_grams_after_another_metric_weight() {
        let stripped = strip_ingredients(vec![ingredient(
            "flour",
            &[("1", "cup"), ("0.25", "kg"), ("250", "g")],
        )]);
        assert_eq!(stripped.removed, 0);
    }
}
