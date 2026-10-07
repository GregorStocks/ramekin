//! Data migrations written in Rust, for changes SQL can't express without
//! duplicating server logic (anything that needs the ingredient parser or
//! catalog). Each runs once, at startup after the SQL migrations, in its own
//! transaction together with the `data_migrations` row that records it, and a
//! failure stops startup.
//!
//! Migrations use the current models and Diesel DSL, so one that has run in
//! production can be deleted once it no longer compiles; its row stays.

use crate::models::{derived_grams, Ingredient, NewRecipeVersion, RecipeVersion};
use crate::recipes::{create_new_version_cas, TagSource};
use crate::schema::{data_migrations, recipe_versions, recipes};
use anyhow::{anyhow, Context};
use diesel::pg::PgConnection;
use diesel::prelude::*;
use ramekin_core::catalog::{is_volume_unit, Learned};
use ramekin_core::ingredient_parser::detect_section_header;
use serde::Deserialize;
use std::collections::HashSet;

type DataMigration = fn(&mut PgConnection) -> anyhow::Result<()>;

/// Every data migration, in the order they run. Never rename one that has run.
const DATA_MIGRATIONS: &[(&str, DataMigration)] = &[
    ("strip_materialized_grams", strip_materialized_grams),
    ("promote_section_headers", promote_section_headers),
];

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
        .map(|mut ingredient| {
            let Some((last, rest)) = ingredient.measurements.split_last() else {
                return ingredient;
            };
            if rest.is_empty() || last.unit.as_deref() != Some("g") {
                return ingredient;
            }
            // Catalog only: the stored grams came from the catalog at ingest.
            let derived = derived_grams(
                &ingredient.item,
                ingredient.note.as_deref(),
                rest,
                &Learned::new(),
            );
            if derived.is_some_and(|derived| derived.amount == last.amount) {
                removed += 1;
                ingredient.measurements.pop();
            } else if rest.iter().any(|m| {
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
        let ingredients = Vec::<Ingredient>::deserialize(&current.ingredients)
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

/// What promoting stored section headers did to one recipe's ingredients.
struct Promoted {
    ingredients: Vec<Ingredient>,
    /// Header lines turned into the section of the lines after them.
    headers: usize,
}

/// Turn each stored line that is really a section header ("To serve:",
/// "FOR THE GLAZE") into the section of the unsectioned lines after it, the
/// way the ingredient parser does at import. A line only counts when it has
/// no measurements and the next line has no section yet; a header with
/// nothing after it is left alone.
fn promote_headers(ingredients: Vec<Ingredient>) -> Promoted {
    let mut headers = 0;
    let mut current: Option<String> = None;
    let mut promoted = Vec::with_capacity(ingredients.len());
    let mut lines = ingredients.into_iter().peekable();
    while let Some(mut ingredient) = lines.next() {
        if ingredient.section.is_some() {
            current = None;
        }
        let header = (ingredient.measurements.is_empty()
            && ingredient.section.is_none()
            && lines.peek().is_some_and(|next| next.section.is_none()))
        .then(|| detect_section_header(&ingredient.item))
        .flatten();
        if let Some(name) = header {
            current = Some(name);
            headers += 1;
            continue;
        }
        if ingredient.section.is_none() {
            ingredient.section.clone_from(&current);
        }
        promoted.push(ingredient);
    }
    Promoted {
        ingredients: promoted,
        headers,
    }
}

/// Recipes imported before the parser recognized a header (or through a path
/// that lost its colon) store it as an ingredient with no amount. Every live
/// recipe whose current version has one gets a new version (source
/// "migration") with the header moved into the following lines' section.
/// Earlier versions are kept.
fn promote_section_headers(conn: &mut PgConnection) -> anyhow::Result<()> {
    let versions: Vec<RecipeVersion> = recipes::table
        .inner_join(
            recipe_versions::table
                .on(recipes::current_version_id.eq(recipe_versions::id.nullable())),
        )
        .filter(recipes::deleted_at.is_null())
        .select(RecipeVersion::as_select())
        .load(conn)?;

    let (mut recipes_updated, mut headers) = (0, 0);
    for current in &versions {
        let ingredients = Vec::<Ingredient>::deserialize(&current.ingredients)
            .with_context(|| format!("recipe version {} has invalid ingredients", current.id))?;
        let promoted = promote_headers(ingredients);
        if promoted.headers == 0 {
            continue;
        }
        create_new_version_cas(
            conn,
            &NewRecipeVersion {
                ingredients: serde_json::to_value(&promoted.ingredients)?,
                ..NewRecipeVersion::copy_of(current, "migration")
            },
            Some(current.id),
            TagSource::CopyFrom(current.id),
        )
        .map_err(|e| anyhow!("failed to save recipe {}: {e}", current.recipe_id))?;
        recipes_updated += 1;
        headers += promoted.headers;
    }
    tracing::info!(
        recipes_checked = versions.len(),
        recipes_updated,
        headers_promoted = headers,
        "promoted stored section headers"
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

    fn sections(promoted: &Promoted) -> Vec<(String, Option<String>)> {
        promoted
            .ingredients
            .iter()
            .map(|i| (i.item.clone(), i.section.clone()))
            .collect()
    }

    fn line(item: &str, section: Option<&str>) -> (String, Option<String>) {
        (item.to_string(), section.map(str::to_string))
    }

    #[test]
    fn promotes_a_header_into_the_following_lines_section() {
        let promoted = promote_headers(vec![
            ingredient("flour", &[("2", "cup")]),
            ingredient("To serve:", &[]),
            ingredient("lime wedges", &[]),
            ingredient("cilantro", &[("1", "cup")]),
        ]);
        assert_eq!(promoted.headers, 1);
        assert_eq!(
            sections(&promoted),
            vec![
                line("flour", None),
                line("lime wedges", Some("To Serve")),
                line("cilantro", Some("To Serve")),
            ]
        );
        // A second pass finds nothing left to promote.
        assert_eq!(promote_headers(promoted.ingredients).headers, 0);
    }

    #[test]
    fn stops_at_lines_that_already_have_a_section() {
        let mut sectioned = ingredient("sugar", &[("1", "cup")]);
        sectioned.section = Some("Glaze".to_string());
        let promoted = promote_headers(vec![
            ingredient("FILLING", &[]),
            ingredient("apples", &[("4", "")]),
            sectioned,
            ingredient("salt", &[]),
        ]);
        assert_eq!(
            sections(&promoted),
            vec![
                line("apples", Some("Filling")),
                line("sugar", Some("Glaze")),
                line("salt", None),
            ]
        );
    }

    #[test]
    fn leaves_ingredients_and_trailing_headers_alone() {
        let mut sectioned = ingredient("butter", &[("1", "tbsp")]);
        sectioned.section = Some("Sauce".to_string());
        let promoted = promote_headers(vec![
            ingredient("Salt", &[]),
            ingredient("Chipotle Sour Cream", &[]),
            ingredient("sour cream", &[("1", "cup")]),
            // The next line already has a section.
            ingredient("For the sauce:", &[]),
            sectioned,
            // Nothing after it.
            ingredient("To serve:", &[]),
        ]);
        assert_eq!(promoted.headers, 0);
        assert_eq!(promoted.ingredients.len(), 6);
    }
}
