//! Normalize ops on the server: the recipe a run starts from, the recipe its
//! ops' step outputs turn that into, and how a version records the ops.
//!
//! The ops themselves are defined once in `ramekin_core::pipeline::NormalizeOp`.

use diesel::prelude::*;
use ramekin_core::pipeline::{
    deserialize_optional_output_field, deserialize_required_output_field,
    steps::{ExtractRecipeStep, FetchImagesStepMeta, ParseIngredientsStep},
    NormalizeOp, StepOutputStore,
};
use ramekin_core::RawRecipe;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::api::recipes::paprika::format_ingredients_text;
use crate::ids::{PhotoIds, Uuid};
use crate::models::{Ingredient, NewRecipeVersion, RecipeVersion};
use crate::recipes::{create_new_version_cas, TagSource, VersionWriteError};
use crate::schema::recipe_versions;
use crate::types::RecipeContent;

/// `version_source` of a version written by normalize ops.
pub const VERSION_SOURCE: &str = "normalize";

/// Ops as stored in `normalize_ops` columns: a JSON array of op ids.
pub fn ops_json(ops: &[NormalizeOp]) -> Value {
    serde_json::to_value(ops).expect("normalize ops serialize to JSON")
}

pub fn ops_from_json(value: &Value) -> Result<Vec<NormalizeOp>, String> {
    Vec::<NormalizeOp>::deserialize(value).map_err(|e| format!("Malformed normalize ops: {e}"))
}

/// The op ids a version's `normalize_ops` column records, for API responses.
pub fn version_op_ids(normalize_ops: Option<&Value>) -> Vec<String> {
    normalize_ops
        .and_then(|ops| Vec::<String>::deserialize(ops).ok())
        .unwrap_or_default()
}

/// Add the `suggested` tags `tags` doesn't already have, ignoring case.
pub fn merge_tags(tags: &mut Vec<String>, suggested: Vec<String>) {
    for tag in suggested {
        if !tags
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&tag))
        {
            tags.push(tag);
        }
    }
}

/// The tags in `proposed` that `base` doesn't have, ignoring case.
pub fn added_tags(base: &[String], proposed: &[String]) -> Vec<String> {
    proposed
        .iter()
        .filter(|tag| {
            !base
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(tag))
        })
        .cloned()
        .collect()
}

/// A recipe's content and photos: what a normalize run starts from, and what
/// it proposes.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NormalizedRecipe {
    pub content: RecipeContent,
    pub photo_ids: Vec<Uuid>,
}

impl NormalizedRecipe {
    pub fn from_version(version: &RecipeVersion, tags: Vec<String>) -> Result<Self, String> {
        let ingredients: Vec<Ingredient> = serde_json::from_value(version.ingredients.clone())
            .map_err(|e| format!("Malformed ingredients on version {}: {e}", version.id))?;
        Ok(Self {
            content: RecipeContent {
                title: version.title.clone(),
                description: version.description.clone(),
                ingredients,
                instructions: version.instructions.clone(),
                source_url: version.source_url.clone(),
                source_name: version.source_name.clone(),
                tags,
                servings: version.servings.clone(),
                prep_time: version.prep_time.clone(),
                cook_time: version.cook_time.clone(),
                total_time: version.total_time.clone(),
                rating: version.rating,
                difficulty: version.difficulty.clone(),
                nutritional_info: version.nutritional_info.clone(),
                notes: version.notes.clone(),
            },
            photo_ids: version.photo_ids.iter().flatten().copied().collect(),
        })
    }

    /// The recipe as the `raw_recipe` that normalize steps read.
    pub fn to_raw_recipe(&self) -> RawRecipe {
        let content = &self.content;
        RawRecipe {
            title: content.title.clone(),
            description: content.description.clone(),
            ingredients: format_ingredients_text(&content.ingredients),
            instructions: content.instructions.clone(),
            image_urls: Vec::new(),
            source_url: content.source_url.clone(),
            source_name: content.source_name.clone(),
            servings: content.servings.clone(),
            prep_time: content.prep_time.clone(),
            cook_time: content.cook_time.clone(),
            total_time: content.total_time.clone(),
            rating: content.rating,
            difficulty: content.difficulty.clone(),
            nutritional_info: content.nutritional_info.clone(),
            notes: content.notes.clone(),
            categories: None,
            footnotes: None,
        }
    }
}

/// Load a version and its tags as a [`NormalizedRecipe`].
pub fn load_version(
    conn: &mut diesel::SqliteConnection,
    version_id: Uuid,
) -> Result<NormalizedRecipe, String> {
    let (version, tags): (RecipeVersion, Vec<String>) = recipe_versions::table
        .find(version_id)
        .select((RecipeVersion::as_select(), crate::raw_sql::tags_subquery()))
        .first(conn)
        .map_err(|e| format!("Failed to load version {version_id}: {e}"))?;
    NormalizedRecipe::from_version(&version, tags)
}

/// [`load_version`] off the runtime threads.
pub async fn load_version_blocking(
    pool: &crate::db::DbPool,
    version_id: Uuid,
) -> Result<NormalizedRecipe, String> {
    crate::db::run_blocking(pool, move |conn| load_version(conn, version_id))
        .await
        .map_err(|e| e.to_string())?
}

/// Save `recipe` as the recipe's new current version, recording `ops`, if
/// its current version is still `expected_version_id`. Its tags are that
/// version's tag links, so tags renamed or deleted since keep that change,
/// plus `added_tags`. Queues its unknown
/// ingredient names; call `ingredient_names::wake` once this commits. Call
/// inside `db::write`.
pub fn write_version(
    conn: &mut diesel::SqliteConnection,
    recipe_id: Uuid,
    user_id: Uuid,
    expected_version_id: Uuid,
    recipe: &NormalizedRecipe,
    added_tags: &[String],
    ops: &[NormalizeOp],
) -> Result<Uuid, VersionWriteError> {
    let content = &recipe.content;
    let ingredients =
        serde_json::to_value(&content.ingredients).expect("ingredients serialize to JSON");
    let photo_ids: PhotoIds = recipe.photo_ids.iter().map(|id| Some(*id)).collect();
    let new_version = NewRecipeVersion {
        id: Uuid::now_v7(),
        recipe_id,
        title: &content.title,
        description: content.description.as_deref(),
        ingredients,
        instructions: &content.instructions,
        source_url: content.source_url.as_deref(),
        source_name: content.source_name.as_deref(),
        photo_ids: &photo_ids,
        servings: content.servings.as_deref(),
        prep_time: content.prep_time.as_deref(),
        cook_time: content.cook_time.as_deref(),
        total_time: content.total_time.as_deref(),
        rating: content.rating,
        difficulty: content.difficulty.as_deref(),
        nutritional_info: content.nutritional_info.as_deref(),
        notes: content.notes.as_deref(),
        version_source: VERSION_SOURCE,
        normalize_ops: Some(ops_json(ops)),
    };
    let version_id = create_new_version_cas(
        conn,
        &new_version,
        Some(expected_version_id),
        TagSource::CopyAndNames {
            from_version: expected_version_id,
            user_id,
            names: added_tags,
        },
    )?;
    crate::ingredient_names::enqueue_items(
        conn,
        content.ingredients.iter().map(|i| i.item.as_str()),
    )?;
    Ok(version_id)
}

/// The recipe a source op rebuilt from this job's `extract_recipe`,
/// `parse_ingredients` and (after a refetch) `fetch_images` outputs. Tags
/// carry over from `current`, as do its photos when the job fetched none.
pub async fn source_recipe(
    outputs: &dyn StepOutputStore,
    current: &NormalizedRecipe,
) -> Result<NormalizedRecipe, String> {
    let extract = outputs
        .get_output(ExtractRecipeStep::NAME)
        .await
        .ok_or("extract_recipe output not found")?;
    let raw: RawRecipe =
        deserialize_required_output_field(&extract, ExtractRecipeStep::NAME, "raw_recipe")?;
    let parsed = outputs.get_output(ParseIngredientsStep::NAME).await;
    let ingredients: Vec<Ingredient> = deserialize_optional_output_field(
        parsed.as_ref(),
        ParseIngredientsStep::NAME,
        "ingredients",
    )?
    .ok_or("parse_ingredients output not found")?;
    let fetched = outputs.get_output(FetchImagesStepMeta::NAME).await;
    let photo_ids: Option<Vec<Uuid>> = deserialize_optional_output_field(
        fetched.as_ref(),
        FetchImagesStepMeta::NAME,
        "photo_ids",
    )?;
    Ok(NormalizedRecipe {
        content: RecipeContent {
            title: raw.title,
            description: raw.description,
            ingredients,
            instructions: raw.instructions,
            source_url: raw
                .source_url
                .or_else(|| current.content.source_url.clone()),
            source_name: raw.source_name,
            tags: current.content.tags.clone(),
            servings: raw.servings,
            prep_time: raw.prep_time,
            cook_time: raw.cook_time,
            total_time: raw.total_time,
            rating: raw.rating,
            difficulty: raw.difficulty,
            nutritional_info: raw.nutritional_info,
            notes: raw.notes,
        },
        photo_ids: photo_ids
            .filter(|ids| !ids.is_empty())
            .unwrap_or_else(|| current.photo_ids.clone()),
    })
}

/// Apply each normalize op's step output to `content`. An op whose step
/// reports no change leaves it as it was.
pub async fn apply_op_outputs(
    content: &mut RecipeContent,
    ops: &[NormalizeOp],
    outputs: &dyn StepOutputStore,
) -> Result<(), String> {
    for op in ops {
        let Some(step_name) = op.step_name() else {
            continue;
        };
        let output = outputs
            .get_output(step_name)
            .await
            .ok_or_else(|| format!("{step_name} output not found"))?;
        match op {
            NormalizeOp::Refetch | NormalizeOp::Reextract => {}
            NormalizeOp::NormalizeTitle => {
                if changed(&output) {
                    content.title =
                        deserialize_required_output_field(&output, step_name, "normalized_title")?;
                }
            }
            NormalizeOp::GenerateDescription => {
                if changed(&output) {
                    content.description = Some(deserialize_required_output_field(
                        &output,
                        step_name,
                        "generated_description",
                    )?);
                }
            }
            NormalizeOp::AutoTag => {
                let suggested =
                    deserialize_required_output_field(&output, step_name, "suggested_tags")?;
                merge_tags(&mut content.tags, suggested);
            }
        }
    }
    Ok(())
}

fn changed(output: &Value) -> bool {
    output
        .get("changed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use async_trait::async_trait;
    use serde_json::json;

    use super::*;

    struct Outputs(HashMap<String, Value>);

    #[async_trait]
    impl StepOutputStore for Outputs {
        async fn get_output(&self, step_name: &str) -> Option<Value> {
            self.0.get(step_name).cloned()
        }

        async fn save_output(
            &mut self,
            _step_name: &str,
            _output: &Value,
            _duration_ms: i64,
            _success: bool,
            _error: Option<&str>,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            unreachable!("read-only test output store")
        }
    }

    fn content() -> RecipeContent {
        serde_json::from_value(json!({
            "title": "soup",
            "description": "Old",
            "ingredients": [],
            "instructions": "Boil.",
            "tags": ["Dinner"],
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn applies_changed_outputs_and_merges_tags() {
        let outputs = Outputs(HashMap::from([
            (
                "normalize_title".to_string(),
                json!({ "changed": true, "normalized_title": "Soup" }),
            ),
            (
                "generate_description".to_string(),
                json!({ "changed": false, "generated_description": "Old" }),
            ),
            (
                "auto_tag".to_string(),
                json!({ "suggested_tags": ["dinner", "Quick"] }),
            ),
        ]));
        let mut recipe = content();
        apply_op_outputs(
            &mut recipe,
            &[
                NormalizeOp::Reextract,
                NormalizeOp::NormalizeTitle,
                NormalizeOp::GenerateDescription,
                NormalizeOp::AutoTag,
            ],
            &outputs,
        )
        .await
        .unwrap();
        assert_eq!(recipe.title, "Soup");
        assert_eq!(recipe.description.as_deref(), Some("Old"));
        assert_eq!(recipe.tags, vec!["Dinner", "Quick"]);
    }

    #[tokio::test]
    async fn missing_op_output_is_an_error() {
        let outputs = Outputs(HashMap::new());
        let err = apply_op_outputs(&mut content(), &[NormalizeOp::AutoTag], &outputs)
            .await
            .unwrap_err();
        assert_eq!(err, "auto_tag output not found");
    }

    #[test]
    fn ops_round_trip_through_json() {
        let ops = [NormalizeOp::Reextract, NormalizeOp::AutoTag];
        assert_eq!(ops_json(&ops), json!(["reextract", "auto_tag"]));
        assert_eq!(ops_from_json(&ops_json(&ops)).unwrap(), ops);
    }
}
