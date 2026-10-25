//! Normalize ops: list them, run any subset on a recipe as a job that
//! proposes a new version, and accept the proposal.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use ramekin_core::pipeline::{order_ops, NormalizeOp, NormalizeOpGroup, PROPOSE_NORMALIZE_STEP};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::api::recipes::read::fetch_current_recipe_with_version;
use crate::api::{run_db, run_db_write, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::ids::Uuid;
use crate::normalize::{load_version, ops_from_json, write_version, NormalizedRecipe};
use crate::recipes::VersionWriteError;
use crate::schema::scrape_jobs;
use crate::scraping::output_store::latest_output;
use crate::scraping::steps::NormalizeProposal;
use crate::scraping::{self, NormalizeStart, StoredSource};

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NormalizeOpGroupResponse {
    /// Replaces the base content; runs first.
    Source,
    /// Transforms the current content.
    Normalize,
}

/// One normalize op, as the registry defines it.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NormalizeOpInfo {
    pub id: String,
    pub label: String,
    pub group: NormalizeOpGroupResponse,
    pub uses_ai: bool,
    /// Recipe fields the op can change.
    pub writes: Vec<String>,
    /// Ops this one runs after when both run.
    pub runs_after: Vec<String>,
    /// Ops that always run with this one.
    pub implies: Vec<String>,
    /// Whether imports and scrapes run the op automatically.
    pub default_on_import: bool,
    /// Whether the op can run on the recipe asked about; always true without
    /// a recipe.
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NormalizeOpsResponse {
    /// Every op, in run order.
    pub ops: Vec<NormalizeOpInfo>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct NormalizeOpsParams {
    /// Report which ops can run on this recipe.
    pub recipe_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NormalizeRequest {
    /// Op ids to run. Implied ops are added, and the registry orders them.
    pub ops: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkNormalizeRequest {
    pub recipe_ids: Vec<Uuid>,
    /// Op ids to run on each recipe.
    pub ops: Vec<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NormalizeJobResponse {
    /// The job; poll it with `GET /api/scrape/{id}`.
    pub job_id: Uuid,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BulkNormalizeResult {
    pub recipe_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<Uuid>,
    /// Why no job started for this recipe.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BulkNormalizeResponse {
    pub results: Vec<BulkNormalizeResult>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NormalizeProposalResponse {
    /// The job's status, as `GET /api/scrape/{id}` reports it.
    pub status: String,
    /// The ops the job runs, in run order.
    pub ops: Vec<String>,
    /// The version the job started from.
    pub base_version_id: Uuid,
    pub base: NormalizedRecipe,
    /// What accepting saves; absent until the job completes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposed: Option<NormalizedRecipe>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AcceptNormalizeResponse {
    /// The new current version.
    pub version_id: Uuid,
}

fn op_ids(ops: &[NormalizeOp]) -> Vec<String> {
    ops.iter().map(|op| op.id().to_string()).collect()
}

fn parse_ops(ids: &[String]) -> Result<Vec<NormalizeOp>, ApiError> {
    if ids.is_empty() {
        return Err(ApiError::invalid_request("Choose at least one op"));
    }
    let ops = ids
        .iter()
        .map(|id| {
            NormalizeOp::from_id(id)
                .ok_or_else(|| ApiError::invalid_request(format!("Unknown op: {id}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(order_ops(&ops))
}

/// A recipe's current version and what its source ops could run from.
struct RecipeSource {
    recipe_id: Uuid,
    current_version_id: Uuid,
    source_url: Option<String>,
}

impl RecipeSource {
    async fn load(pool: &DbPool, user_id: Uuid, recipe_id: Uuid) -> Result<Self, ApiError> {
        run_db(pool, move |conn| {
            let recipe = fetch_current_recipe_with_version(conn, user_id, recipe_id).map_err(
                |e| match e {
                    diesel::NotFound => ApiError::not_found("Recipe not found"),
                    _ => ApiError::internal("Failed to fetch recipe"),
                },
            )?;
            Ok(Self {
                recipe_id,
                current_version_id: recipe.version.id,
                source_url: recipe.version.source_url,
            })
        })
        .await
    }

    /// The URL a refetch would fetch, or why there is none.
    fn refetch_url(&self) -> Result<String, String> {
        let url = self
            .source_url
            .clone()
            .filter(|url| !url.is_empty())
            .ok_or("Recipe has no source URL")?;
        scraping::is_host_allowed(&url).map_err(|e| e.to_string())?;
        Ok(url)
    }

    /// The source a reextract would rebuild from, or why there is none.
    async fn stored_source(&self, pool: &DbPool) -> Result<Result<StoredSource, String>, ApiError> {
        let source = scraping::stored_source(pool, self.recipe_id)
            .await
            .map_err(|e| {
                tracing::error!("Failed to find stored source: {e}");
                ApiError::internal("Failed to fetch recipe")
            })?;
        Ok(source.ok_or_else(|| "Recipe has no saved source to re-extract from".to_string()))
    }

    /// Where a job running `ops` starts: its first source op's source, or
    /// the current version. 400 when that source op can't run.
    async fn start_for(
        &self,
        pool: &DbPool,
        ops: &[NormalizeOp],
    ) -> Result<NormalizeStart, ApiError> {
        let unavailable = |op: NormalizeOp, reason: String| {
            ApiError::invalid_request(format!("{} is unavailable: {reason}", op.label()))
        };
        // Refetch implies reextract, but rebuilds from the page it fetches.
        if ops.contains(&NormalizeOp::Refetch) {
            let url = self
                .refetch_url()
                .map_err(|reason| unavailable(NormalizeOp::Refetch, reason))?;
            Ok(NormalizeStart::Refetch { url })
        } else if ops.contains(&NormalizeOp::Reextract) {
            let source = self
                .stored_source(pool)
                .await?
                .map_err(|reason| unavailable(NormalizeOp::Reextract, reason))?;
            Ok(NormalizeStart::Reextract(source))
        } else {
            Ok(NormalizeStart::BaseVersion)
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/recipes/normalize-ops",
    tag = "normalize",
    params(NormalizeOpsParams),
    responses(
        (status = 200, description = "Every normalize op, in run order", body = NormalizeOpsResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Recipe not found", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_normalize_ops(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Query(params): Query<NormalizeOpsParams>,
) -> Result<impl IntoResponse, ApiError> {
    let (refetch, reextract) = match params.recipe_id {
        Some(recipe_id) => {
            let source = RecipeSource::load(&pool, user.id, recipe_id).await?;
            (
                source.refetch_url().err(),
                source.stored_source(&pool).await?.err(),
            )
        }
        None => (None, None),
    };
    let ops = NormalizeOp::ALL
        .iter()
        .map(|op| {
            let unavailable_reason = match op {
                NormalizeOp::Refetch => refetch.clone(),
                NormalizeOp::Reextract => reextract.clone(),
                NormalizeOp::NormalizeTitle
                | NormalizeOp::GenerateDescription
                | NormalizeOp::AutoTag => None,
            };
            NormalizeOpInfo {
                id: op.id().to_string(),
                label: op.label().to_string(),
                group: match op.group() {
                    NormalizeOpGroup::Source => NormalizeOpGroupResponse::Source,
                    NormalizeOpGroup::Normalize => NormalizeOpGroupResponse::Normalize,
                },
                uses_ai: op.uses_ai(),
                writes: op.writes().iter().map(|f| f.to_string()).collect(),
                runs_after: op_ids(op.runs_after()),
                implies: op_ids(op.implies()),
                default_on_import: op.default_on_import(),
                available: unavailable_reason.is_none(),
                unavailable_reason,
            }
        })
        .collect();
    Ok(Json(NormalizeOpsResponse { ops }))
}

/// Create and start a normalize job for one recipe.
async fn start_normalize_job(
    pool: &Arc<DbPool>,
    user_id: Uuid,
    recipe_id: Uuid,
    ops: &[NormalizeOp],
) -> Result<NormalizeJobResponse, ApiError> {
    let source = RecipeSource::load(pool, user_id, recipe_id).await?;
    let start = source.start_for(pool, ops).await?;
    let job = scraping::create_normalize_job(
        pool,
        user_id,
        recipe_id,
        source.current_version_id,
        ops,
        start,
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to create normalize job: {e}");
        ApiError::internal("Failed to create normalize job")
    })?;
    scraping::spawn_import_job(pool.clone(), job.id, "normalize");
    Ok(NormalizeJobResponse {
        job_id: job.id,
        status: job.status,
    })
}

#[utoipa::path(
    post,
    path = "/api/recipes/{id}/normalize",
    tag = "normalize",
    params(("id" = Uuid, Path, description = "Recipe ID")),
    request_body = NormalizeRequest,
    responses(
        (status = 201, description = "Normalize job started; it proposes a version", body = NormalizeJobResponse),
        (status = 400, description = "Unknown op, or an op unavailable for this recipe", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Recipe not found", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn normalize_recipe(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path(recipe_id): Path<Uuid>,
    Json(request): Json<NormalizeRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let ops = parse_ops(&request.ops)?;
    let job = start_normalize_job(&pool, user.id, recipe_id, &ops).await?;
    Ok((StatusCode::CREATED, Json(job)))
}

#[utoipa::path(
    post,
    path = "/api/recipes/normalize",
    tag = "normalize",
    request_body = BulkNormalizeRequest,
    responses(
        (status = 200, description = "One result per recipe: its job, or why none started", body = BulkNormalizeResponse),
        (status = 400, description = "Unknown op", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn normalize_recipes(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<BulkNormalizeRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let ops = parse_ops(&request.ops)?;
    let mut results = Vec::with_capacity(request.recipe_ids.len());
    for recipe_id in request.recipe_ids {
        let result = match start_normalize_job(&pool, user.id, recipe_id, &ops).await {
            Ok(job) => BulkNormalizeResult {
                recipe_id,
                job_id: Some(job.job_id),
                error: None,
            },
            Err(error) => BulkNormalizeResult {
                recipe_id,
                job_id: None,
                error: Some(error.message),
            },
        };
        results.push(result);
    }
    Ok(Json(BulkNormalizeResponse { results }))
}

/// The user's normalize job `job_id` on `recipe_id`: its status, ops and
/// base version.
async fn load_normalize_job(
    pool: &DbPool,
    user_id: Uuid,
    recipe_id: Uuid,
    job_id: Uuid,
) -> Result<(String, Vec<NormalizeOp>, Uuid), ApiError> {
    let (status, ops, base_version_id) = run_db(pool, move |conn| {
        scrape_jobs::table
            .find(job_id)
            .filter(scrape_jobs::user_id.eq(user_id))
            .filter(scrape_jobs::recipe_id.eq(recipe_id))
            .select((
                scrape_jobs::status,
                scrape_jobs::normalize_ops,
                scrape_jobs::expected_version_id,
            ))
            .first::<(String, Option<serde_json::Value>, Option<Uuid>)>(conn)
            .map_err(|e| match e {
                diesel::NotFound => ApiError::not_found("Normalize job not found"),
                _ => ApiError::internal("Failed to fetch normalize job"),
            })
    })
    .await?;
    let (Some(ops), Some(base_version_id)) = (ops, base_version_id) else {
        return Err(ApiError::not_found("Normalize job not found"));
    };
    let ops = ops_from_json(&ops).map_err(ApiError::internal)?;
    Ok((status, ops, base_version_id))
}

async fn load_proposal(pool: &DbPool, job_id: Uuid) -> Result<Option<NormalizeProposal>, ApiError> {
    let output = latest_output(pool, job_id, PROPOSE_NORMALIZE_STEP, true)
        .await
        .map_err(|e| {
            tracing::error!("Failed to read normalize proposal: {e:#}");
            ApiError::internal("Failed to read normalize proposal")
        })?;
    output.map(serde_json::from_value).transpose().map_err(|e| {
        tracing::error!("Malformed normalize proposal: {e}");
        ApiError::internal("Failed to read normalize proposal")
    })
}

#[utoipa::path(
    get,
    path = "/api/recipes/{id}/normalize/{job_id}",
    tag = "normalize",
    params(
        ("id" = Uuid, Path, description = "Recipe ID"),
        ("job_id" = Uuid, Path, description = "Normalize job ID")
    ),
    responses(
        (status = 200, description = "The job's base version and, once it completes, its proposal", body = NormalizeProposalResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Normalize job not found", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_normalize_proposal(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path((recipe_id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, ApiError> {
    let (status, ops, base_version_id) =
        load_normalize_job(&pool, user.id, recipe_id, job_id).await?;
    let base = run_db(&pool, move |conn| {
        load_version(conn, base_version_id).map_err(ApiError::internal)
    })
    .await?;
    let proposed = match status.as_str() {
        scraping::STATUS_COMPLETED => load_proposal(&pool, job_id).await?.map(|p| p.proposed),
        _ => None,
    };
    Ok(Json(NormalizeProposalResponse {
        status,
        ops: op_ids(&ops),
        base_version_id,
        base,
        proposed,
    }))
}

#[utoipa::path(
    post,
    path = "/api/recipes/{id}/normalize/{job_id}/accept",
    tag = "normalize",
    params(
        ("id" = Uuid, Path, description = "Recipe ID"),
        ("job_id" = Uuid, Path, description = "Normalize job ID")
    ),
    responses(
        (status = 201, description = "Proposal saved as the recipe's new version", body = AcceptNormalizeResponse),
        (status = 400, description = "The job has not finished proposing a version", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Normalize job not found", body = ErrorResponse),
        (status = 409, description = "The recipe changed since the job started", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn accept_normalize_proposal(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Path((recipe_id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;
    let (status, ops, base_version_id) =
        load_normalize_job(&pool, user_id, recipe_id, job_id).await?;
    let proposal = match status.as_str() {
        scraping::STATUS_COMPLETED => load_proposal(&pool, job_id).await?,
        _ => None,
    }
    .ok_or_else(|| ApiError::invalid_request("The normalize job has no proposal yet"))?;

    let version_id = run_db_write(&pool, move |conn| {
        write_version(
            conn,
            recipe_id,
            user_id,
            base_version_id,
            &proposal.proposed,
            &ops,
        )
        .map_err(|e| match e {
            VersionWriteError::Stale => ApiError::conflict(
                "The recipe changed since this normalize run started; run it again",
            ),
            VersionWriteError::Db(e) => {
                tracing::error!("Failed to save normalized version: {e}");
                ApiError::internal("Failed to save normalized version")
            }
        })
    })
    .await?;
    crate::ingredient_names::wake();
    Ok((
        StatusCode::CREATED,
        Json(AcceptNormalizeResponse { version_id }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ops_orders_and_adds_implied_ops() {
        let ops = parse_ops(&["auto_tag".to_string(), "refetch".to_string()]).unwrap();
        assert_eq!(
            ops,
            vec![
                NormalizeOp::Refetch,
                NormalizeOp::Reextract,
                NormalizeOp::AutoTag
            ]
        );
    }

    #[test]
    fn parse_ops_rejects_unknown_and_empty() {
        assert!(parse_ops(&["enrich".to_string()]).is_err());
        assert!(parse_ops(&[]).is_err());
    }
}
