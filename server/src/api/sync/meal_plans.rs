use super::{patch_text, refused, text_or_none, with_refused, OpKind, OpResult, Target, Window};
use crate::api::meal_plans::list::MealType;
use crate::api::{run_db_write, ApiError, ErrorResponse};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::ids::Uuid;
use crate::models::NewMealPlan;
use crate::schema::{meal_plans, recipe_versions, recipes};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{NaiveDate, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

/// A meal plan's writable fields. On `create`, `recipe_id`, `meal_date` and
/// `meal_type` are required. On `update`, only the fields present change,
/// and `recipe_id` can't. `notes` is cleared with "".
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct MealPlanFields {
    pub recipe_id: Option<Uuid>,
    pub meal_date: Option<NaiveDate>,
    pub meal_type: Option<MealType>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct MealPlanOp {
    pub op: OpKind,
    /// The plan's id. A create chooses it.
    pub id: Uuid,
    #[serde(default)]
    pub fields: MealPlanFields,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SyncMealPlansRequest {
    /// `cursor` from the previous sync. Absent means a full sync.
    pub cursor: Option<i64>,
    /// Writes to apply, in order, before reading changes.
    #[serde(default)]
    pub ops: Vec<MealPlanOp>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncMealPlan {
    pub id: Uuid,
    pub recipe_id: Uuid,
    /// The recipe's current title and first photo. A new recipe version
    /// resends the plans that use it.
    pub recipe_title: String,
    pub thumbnail_photo_id: Option<Uuid>,
    pub meal_date: NaiveDate,
    pub meal_type: MealType,
    pub notes: Option<String>,
    /// Sync stamp of the plan's own last change. A cached copy with a higher
    /// one is newer than this row.
    pub change_seq: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SyncMealPlansResponse {
    /// One per request op, in order.
    pub results: Vec<OpResult>,
    /// Live plans (of live recipes) changed at or after the request's
    /// `cursor`, this request's own writes and every refused op's live plan
    /// included; every live plan on a full resync.
    pub changes: Vec<SyncMealPlan>,
    /// Plan IDs deleted, or whose recipe was deleted, at or after `cursor`,
    /// and every refused op's plan that isn't live. Only the latter on a full
    /// resync.
    pub deleted: Vec<Uuid>,
    /// Pass as `cursor` on the next sync.
    pub cursor: i64,
    /// True when `changes` holds every live plan: drop cached plans it omits.
    pub full_resync: bool,
}

type PlanRow = (
    Uuid,
    Uuid,
    String,
    crate::ids::PhotoIds,
    NaiveDate,
    String,
    Option<String>,
    i64,
);

#[utoipa::path(
    post,
    path = "/api/sync/meal-plans",
    tag = "sync",
    request_body = SyncMealPlansRequest,
    responses(
        (status = 200, description = "Ops applied; changes since the cursor", body = SyncMealPlansResponse),
        (status = 400, description = "A malformed op", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    security(("bearer_auth" = []))
)]
pub async fn sync_meal_plans(
    AuthUser(user): AuthUser,
    State(pool): State<Arc<DbPool>>,
    Json(request): Json<SyncMealPlansRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = user.id;

    let response = run_db_write(&pool, move |conn| {
        let ids: Vec<Uuid> = request.ops.iter().map(|op| op.id).collect();
        let mut results = Vec::with_capacity(request.ops.len());

        for op in request.ops {
            let owner: Option<(Uuid, bool)> = meal_plans::table
                .find(op.id)
                .select((meal_plans::user_id, meal_plans::deleted_at.is_null()))
                .first(conn)
                .optional()?;
            let target = Target::of(owner, user_id);

            let result = match op.op {
                OpKind::Create => match target.existing_create() {
                    Some(result) => result,
                    None => create(conn, user_id, op.id, op.fields)?,
                },
                OpKind::Update => {
                    // A plan whose recipe is deleted is gone from the feed.
                    let current: Option<(Uuid, NaiveDate, String, Option<String>)> =
                        meal_plans::table
                            .inner_join(recipes::table)
                            .filter(meal_plans::id.eq(op.id))
                            .filter(meal_plans::user_id.eq(user_id))
                            .filter(meal_plans::deleted_at.is_null())
                            .filter(recipes::deleted_at.is_null())
                            .select((
                                meal_plans::recipe_id,
                                meal_plans::meal_date,
                                meal_plans::meal_type,
                                meal_plans::notes,
                            ))
                            .first(conn)
                            .optional()?;
                    match current {
                        None => OpResult::Gone,
                        Some((recipe_id, meal_date, meal_type, notes)) => {
                            let fields = op.fields;
                            let meal_date = fields.meal_date.unwrap_or(meal_date);
                            let meal_type = match fields.meal_type {
                                Some(meal_type) => meal_type,
                                None => MealType::from_str(&meal_type)
                                    .ok_or_else(|| ApiError::internal("Corrupt meal type"))?,
                            };
                            if slot_taken(
                                conn,
                                user_id,
                                Some(op.id),
                                recipe_id,
                                meal_date,
                                meal_type,
                            )? {
                                OpResult::Duplicate
                            } else {
                                diesel::update(meal_plans::table.find(op.id))
                                    .set((
                                        meal_plans::meal_date.eq(meal_date),
                                        meal_plans::meal_type.eq(meal_type.as_str()),
                                        meal_plans::notes.eq(patch_text(fields.notes, notes)),
                                    ))
                                    .execute(conn)?;
                                OpResult::Applied
                            }
                        }
                    }
                }
                OpKind::Delete => {
                    if target == (Target::Mine { live: true }) {
                        diesel::update(meal_plans::table.find(op.id))
                            .set(meal_plans::deleted_at.eq(Utc::now()))
                            .execute(conn)?;
                    }
                    target.delete()
                }
            };
            results.push(result);
        }

        // After the writes, so they come back as changes now rather than on
        // the next sync. The write lock keeps anything else from committing
        // in between.
        let window = Window::read(conn, request.cursor)?;
        let refused = refused(&ids, &results);

        let mut changed = meal_plans::table
            .inner_join(recipes::table)
            .inner_join(
                recipe_versions::table.on(recipe_versions::id
                    .nullable()
                    .eq(recipes::current_version_id)),
            )
            .filter(meal_plans::user_id.eq(user_id))
            .filter(meal_plans::deleted_at.is_null())
            .filter(recipes::deleted_at.is_null())
            .into_boxed();
        if let Some(since) = window.since {
            changed = changed.filter(
                meal_plans::change_seq
                    .ge(since)
                    .or(recipe_versions::change_seq.ge(since))
                    .or(meal_plans::id.eq_any(&refused)),
            );
        }
        let rows: Vec<PlanRow> = changed
            .select((
                meal_plans::id,
                meal_plans::recipe_id,
                recipe_versions::title,
                recipe_versions::photo_ids,
                meal_plans::meal_date,
                meal_plans::meal_type,
                meal_plans::notes,
                meal_plans::change_seq,
            ))
            .order((meal_plans::meal_date.asc(), meal_plans::id.asc()))
            .load(conn)?;
        let changes: Vec<SyncMealPlan> = rows
            .into_iter()
            .filter_map(
                |(
                    id,
                    recipe_id,
                    recipe_title,
                    photo_ids,
                    meal_date,
                    meal_type,
                    notes,
                    change_seq,
                )| {
                    Some(SyncMealPlan {
                        id,
                        recipe_id,
                        recipe_title,
                        thumbnail_photo_id: photo_ids.into_iter().flatten().next(),
                        meal_date,
                        meal_type: MealType::from_str(&meal_type)?,
                        notes,
                        change_seq,
                    })
                },
            )
            .collect();

        let deleted = match window.since {
            Some(since) => meal_plans::table
                .inner_join(recipes::table)
                .filter(meal_plans::user_id.eq(user_id))
                .filter(
                    meal_plans::deleted_at
                        .is_not_null()
                        .and(meal_plans::change_seq.ge(since))
                        .or(meal_plans::deleted_at
                            .is_null()
                            .and(recipes::deleted_seq.ge(since))),
                )
                .select(meal_plans::id)
                .load(conn)?,
            None => Vec::new(),
        };
        let live: Vec<Uuid> = changes.iter().map(|plan| plan.id).collect();
        let deleted = with_refused(deleted, &refused, &live);

        Ok(SyncMealPlansResponse {
            results,
            changes,
            deleted,
            cursor: window.cursor,
            full_resync: window.full_resync(),
        })
    })
    .await?;

    Ok((StatusCode::OK, Json(response)))
}

/// Insert a new plan, unless its recipe is gone or already in that meal.
fn create(
    conn: &mut SqliteConnection,
    user_id: Uuid,
    id: Uuid,
    fields: MealPlanFields,
) -> Result<OpResult, ApiError> {
    let (Some(recipe_id), Some(meal_date), Some(meal_type)) =
        (fields.recipe_id, fields.meal_date, fields.meal_type)
    else {
        return Err(ApiError::invalid_request(
            "A create needs a recipe, date and meal",
        ));
    };
    if !recipe_is_live(conn, user_id, recipe_id)? {
        return Ok(OpResult::RecipeMissing);
    }
    if slot_taken(conn, user_id, None, recipe_id, meal_date, meal_type)? {
        return Ok(OpResult::Duplicate);
    }
    let notes = text_or_none(fields.notes);
    diesel::insert_into(meal_plans::table)
        .values(NewMealPlan {
            id,
            user_id,
            recipe_id,
            meal_date,
            meal_type: meal_type.as_str(),
            notes: notes.as_deref(),
        })
        .execute(conn)?;
    Ok(OpResult::Applied)
}

fn recipe_is_live(
    conn: &mut SqliteConnection,
    user_id: Uuid,
    recipe_id: Uuid,
) -> QueryResult<bool> {
    diesel::select(diesel::dsl::exists(
        recipes::table
            .filter(recipes::id.eq(recipe_id))
            .filter(recipes::user_id.eq(user_id))
            .filter(recipes::deleted_at.is_null()),
    ))
    .get_result(conn)
}

/// Whether another live plan (than `except`) already has this recipe for
/// this meal.
fn slot_taken(
    conn: &mut SqliteConnection,
    user_id: Uuid,
    except: Option<Uuid>,
    recipe_id: Uuid,
    meal_date: NaiveDate,
    meal_type: MealType,
) -> QueryResult<bool> {
    let mut query = meal_plans::table
        .filter(meal_plans::user_id.eq(user_id))
        .filter(meal_plans::deleted_at.is_null())
        .filter(meal_plans::recipe_id.eq(recipe_id))
        .filter(meal_plans::meal_date.eq(meal_date))
        .filter(meal_plans::meal_type.eq(meal_type.as_str()))
        .into_boxed();
    if let Some(except) = except {
        query = query.filter(meal_plans::id.ne(except));
    }
    diesel::select(diesel::dsl::exists(query)).get_result(conn)
}
