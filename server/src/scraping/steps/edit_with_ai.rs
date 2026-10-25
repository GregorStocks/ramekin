use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use ramekin_core::ai::{edit_recipe, render_recipe, AiConfig, CachingAiClient, RecipeEditContext};
use ramekin_core::pipeline::{
    PipelineStep, StepContext, StepMetadata, StepResult, EDIT_WITH_AI_STEP,
};

use crate::api::recipes::list::search_word_filter;
use crate::db::{run_blocking, DbPool};
use crate::ids::Uuid;
use crate::normalize::load_version_blocking;
use crate::photos::load_photo_images;
use crate::schema::{recipe_versions, recipes, user_tags};
use crate::types::RecipeContent;

use super::helpers::step_failure;

/// What the user asked an Edit with AI job for (`scrape_jobs.edit_request`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditRequest {
    pub instruction: String,
    #[serde(default)]
    pub photo_ids: Vec<Uuid>,
}

/// The output of `edit_with_ai`: the edited recipe and what changed.
#[derive(Debug, Serialize, Deserialize)]
pub struct EditWithAiOutput {
    pub recipe: RecipeContent,
    pub summary: String,
}

/// How many of the user's recipes `search_recipes` returns.
const SEARCH_LIMIT: i64 = 5;

/// The user's tags and recipes, for the edit's tools.
struct UserRecipes {
    pool: Arc<DbPool>,
    user_id: Uuid,
    /// The recipe being edited, which searches leave out.
    recipe_id: Uuid,
}

#[async_trait]
impl RecipeEditContext for UserRecipes {
    async fn user_tags(&self) -> Result<Vec<String>, String> {
        let user_id = self.user_id;
        run_blocking(&self.pool, move |conn| {
            user_tags::table
                .filter(user_tags::user_id.eq(user_id))
                .filter(user_tags::deleted_at.is_null())
                .select(user_tags::name)
                .order(user_tags::name.asc())
                .load::<String>(conn)
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?
    }

    async fn search_recipes(&self, query: &str) -> Result<Vec<String>, String> {
        let (user_id, recipe_id) = (self.user_id, self.recipe_id);
        let words: Vec<String> = query.split_whitespace().map(str::to_string).collect();
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let found = run_blocking(&self.pool, move |conn| {
            let mut search = recipes::table
                .inner_join(
                    recipe_versions::table.on(recipe_versions::id
                        .nullable()
                        .eq(recipes::current_version_id)),
                )
                .filter(recipes::user_id.eq(user_id))
                .filter(recipes::deleted_at.is_null())
                .filter(recipes::id.ne(recipe_id))
                .into_boxed();
            // As the cookbook searches: every word, in any text field.
            for word in &words {
                search = search.filter(search_word_filter(word));
            }
            search
                .select((
                    recipe_versions::title,
                    recipe_versions::ingredients,
                    recipe_versions::instructions,
                ))
                .limit(SEARCH_LIMIT)
                .load::<(String, serde_json::Value, String)>(conn)
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())??;
        Ok(found
            .into_iter()
            .map(|(title, ingredients, instructions)| {
                render_recipe(serde_json::json!({
                    "title": title,
                    "ingredients": ingredients,
                    "instructions": instructions,
                }))
            })
            .collect())
    }
}

/// Edits the job's base version the way its request says, with the recipe's
/// photos as context. Saves nothing; `propose_normalize` proposes the result.
pub struct EditWithAiStep {
    pool: Arc<DbPool>,
    user_id: Uuid,
    recipe_id: Uuid,
    base_version_id: Uuid,
    request: EditRequest,
}

impl EditWithAiStep {
    pub fn new(
        pool: Arc<DbPool>,
        user_id: Uuid,
        recipe_id: Uuid,
        base_version_id: Uuid,
        request: EditRequest,
    ) -> Self {
        Self {
            pool,
            user_id,
            recipe_id,
            base_version_id,
            request,
        }
    }
}

#[async_trait]
impl PipelineStep for EditWithAiStep {
    fn metadata(&self) -> StepMetadata {
        StepMetadata {
            name: EDIT_WITH_AI_STEP,
            description: "Edit the recipe with AI",
            continues_on_failure: false,
        }
    }

    async fn execute(&self, _ctx: &StepContext<'_>) -> StepResult {
        let start = Instant::now();
        let fail = |error: String| step_failure(EDIT_WITH_AI_STEP, start, error);
        let base = match load_version_blocking(&self.pool, self.base_version_id).await {
            Ok(recipe) => recipe,
            Err(error) => return fail(error),
        };
        let images =
            match load_photo_images(&self.pool, self.user_id, &self.request.photo_ids).await {
                Ok(images) => images,
                Err(error) => return fail(format!("Couldn't load the photos: {error}")),
            };
        let ai_client = match AiConfig::from_env() {
            Ok(config) => CachingAiClient::new(config.for_recipe_edit()),
            Err(error) => return fail(format!("AI is not configured: {error}")),
        };
        let context = UserRecipes {
            pool: self.pool.clone(),
            user_id: self.user_id,
            recipe_id: self.recipe_id,
        };
        let edited = match edit_recipe(
            &ai_client,
            &base.content,
            &self.request.instruction,
            images,
            &context,
        )
        .await
        {
            Ok(edited) => edited,
            Err(error) => return fail(format!("Edit with AI failed: {error}")),
        };
        tracing::info!(
            turns = edited.turns,
            cached = edited.cached,
            prompt_tokens = edited.usage.prompt_tokens,
            completion_tokens = edited.usage.completion_tokens,
            "Edit with AI finished"
        );
        let output = EditWithAiOutput {
            recipe: edited.recipe,
            summary: edited.summary,
        };
        StepResult {
            step_name: EDIT_WITH_AI_STEP.to_string(),
            success: true,
            output: serde_json::to_value(output).expect("EditWithAiOutput serializes to JSON"),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            next_step: None,
        }
    }
}
