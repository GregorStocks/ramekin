//! Catalog step 3, weights: a calorie estimate reports the weights the catalog
//! lacks for foods it does know (no density, or no weight for a counted unit
//! such as "bunch"), the estimate endpoint queues them, an LLM estimates them
//! in the background, and later estimates read them back, labeled.
//!
//! Each (food, unit) is a row in `ingredient_weight_estimates` carrying its own
//! state, shared across users, worked by the same worker and batch lock as
//! ingredient names (see `ingredient_names`).

use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;
use diesel::prelude::*;
use ramekin_core::ai::{estimate_ingredient_weights, AiError};
use ramekin_core::nutrition::{WeightKey, Weights};

use crate::db::{run_blocking, DbPool};
use crate::ingredient_names::{Step, BATCH, BATCH_SIZE, CLIENT, FAILED, PENDING, RESOLVED};
use crate::schema::ingredient_weight_estimates as weights;

/// Queue weight gaps for estimating. Gaps already queued or estimated are
/// left alone, so concurrent estimates can never fail either.
pub fn enqueue(conn: &mut PgConnection, gaps: &[WeightKey]) -> QueryResult<usize> {
    if gaps.is_empty() {
        return Ok(0);
    }
    let rows: Vec<_> = gaps
        .iter()
        .map(|gap| {
            (
                weights::food.eq(&gap.food),
                weights::unit.eq(&gap.unit),
                weights::status.eq(PENDING),
            )
        })
        .collect();
    diesel::insert_into(weights::table)
        .values(&rows)
        .on_conflict_do_nothing()
        .execute(conn)
}

/// The rows for `keys`, by status. Rows for other units of the same foods are
/// dropped.
fn rows_for(
    conn: &mut PgConnection,
    keys: &[WeightKey],
) -> QueryResult<Vec<(WeightKey, String, Option<f64>)>> {
    let foods: BTreeSet<&str> = keys.iter().map(|key| key.food.as_str()).collect();
    let rows: Vec<(String, String, String, Option<f64>)> = weights::table
        .filter(weights::food.eq_any(foods))
        .select((
            weights::food,
            weights::unit,
            weights::status,
            weights::grams,
        ))
        .load(conn)?;
    Ok(rows
        .into_iter()
        .map(|(food, unit, status, grams)| (WeightKey { food, unit }, status, grams))
        .filter(|(key, _, _)| keys.contains(key))
        .collect())
}

/// The most new gaps one estimate request queues. A recipe rarely has more
/// than a few; with `ESTIMABLE_UNITS` this bounds what any request can add to
/// the paid queue.
const MAX_QUEUED_PER_REQUEST: usize = 50;

/// Estimated weights for `gaps`, and whether any is still waiting: pending,
/// or not yet queued because of `MAX_QUEUED_PER_REQUEST` (the client keeps
/// polling, and each poll queues the next ones). Gaps with no row are queued
/// here, so the caller should `wake` once this commits.
pub fn load_and_enqueue(
    conn: &mut PgConnection,
    gaps: &[WeightKey],
) -> QueryResult<(Weights, bool)> {
    if gaps.is_empty() {
        return Ok((Weights::new(), false));
    }
    let mut rows = rows_for(conn, gaps)?;
    let unqueued: Vec<WeightKey> = gaps
        .iter()
        .filter(|gap| !rows.iter().any(|(key, _, _)| key == *gap))
        .cloned()
        .collect();
    let (queue_now, later) = unqueued.split_at(unqueued.len().min(MAX_QUEUED_PER_REQUEST));
    if !queue_now.is_empty() {
        enqueue(conn, queue_now)?;
        rows = rows_for(conn, gaps)?;
    }
    let waiting = !later.is_empty() || rows.iter().any(|(_, status, _)| status == PENDING);
    let estimated = rows
        .into_iter()
        .filter(|(_, status, _)| status == RESOLVED)
        .filter_map(|(key, _, grams)| grams.map(|grams| (key, grams)))
        .collect();
    Ok((estimated, waiting))
}

/// How the estimates for some foods stand, for the status page.
pub struct Counts {
    pub estimated: i64,
    pub no_typical_weight: i64,
    pub pending: i64,
    pub failed: Vec<(WeightKey, String, i32)>,
}

/// Food, unit, status, grams, error, attempts.
type CountRow = (String, String, String, Option<f64>, Option<String>, i32);

/// Every estimate row for `foods`, whatever its unit: a gap can depend on the
/// scale it was viewed at, so the stored recipes alone can't say which units
/// were queued.
pub fn counts(conn: &mut PgConnection, foods: &[String]) -> QueryResult<Counts> {
    let rows: Vec<CountRow> = weights::table
        .filter(weights::food.eq_any(foods))
        .order(weights::updated_at.desc())
        .select((
            weights::food,
            weights::unit,
            weights::status,
            weights::grams,
            weights::error,
            weights::attempts,
        ))
        .load(conn)?;
    let mut counts = Counts {
        estimated: 0,
        no_typical_weight: 0,
        pending: 0,
        failed: Vec::new(),
    };
    for (food, unit, status, grams, error, attempts) in rows {
        match (status.as_str(), grams) {
            (RESOLVED, Some(_)) => counts.estimated += 1,
            (RESOLVED, None) => counts.no_typical_weight += 1,
            (PENDING, _) => counts.pending += 1,
            _ => counts.failed.push((
                WeightKey { food, unit },
                error.unwrap_or_default(),
                attempts,
            )),
        }
    }
    Ok(counts)
}

/// Put the failed estimates for `foods` back in the queue.
pub fn requeue_failed(conn: &mut PgConnection, foods: &[String]) -> QueryResult<usize> {
    diesel::update(
        weights::table
            .filter(weights::status.eq(FAILED))
            .filter(weights::food.eq_any(foods)),
    )
    .set((
        weights::status.eq(PENDING),
        weights::error.eq(None::<String>),
    ))
    .execute(conn)
}

/// Estimate one batch of pending weights, taken under the shared batch lock.
pub(crate) async fn estimate_next_batch(pool: &Arc<DbPool>) -> Result<Step, String> {
    let _batch = BATCH.lock().await;
    let batch: Vec<(String, String)> = run_blocking(pool, move |conn| {
        weights::table
            .filter(weights::status.eq(PENDING))
            .select((weights::food, weights::unit))
            .order((weights::food, weights::unit))
            .limit(BATCH_SIZE)
            .load(conn)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    if batch.is_empty() {
        return Ok(Step::Idle);
    }
    Ok(match estimate_batch(pool, batch).await? {
        None => Step::Done,
        Some((error, true)) => Step::ProviderFailed(error),
        Some((error, false)) => Step::Failed(error),
    })
}

/// A batch's first failure and whether it was the provider's, or `Err` when an
/// outcome couldn't be saved; boxed, since a failed batch retries its halves.
type BatchFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<(String, bool)>, String>> + Send + 'a>>;

/// Estimate one batch. If the model's answer for a multi-item batch is
/// invalid, each half is retried on its own, down to single items, so one item
/// the model can't handle fails alone in about 2·log2(n) calls rather than n.
/// Returns the first failure and whether it was the provider's (so later
/// batches would fail too); `Err` means an outcome couldn't be saved.
fn estimate_batch(pool: &Arc<DbPool>, batch: Vec<(String, String)>) -> BatchFuture<'_> {
    Box::pin(async move {
        let error = match ask(&batch).await {
            Ok((estimates, model)) => {
                save_estimated(pool, estimates, model).await?;
                return Ok(None);
            }
            Err(AiError::ParseError(error)) if batch.len() > 1 => error,
            Err(error) => return fail(pool, batch, error).await.map(Some),
        };
        tracing::warn!(
            items = batch.len(),
            "ingredient weight batch failed, retrying each half: {}",
            error
        );
        let mut first = batch;
        let second = first.split_off(first.len() / 2);
        let outcome = estimate_batch(pool, first).await?;
        if matches!(outcome, Some((_, true))) {
            return Ok(outcome);
        }
        let later = estimate_batch(pool, second).await?;
        Ok(match (outcome, later) {
            (_, provider @ Some((_, true))) => provider,
            (Some(first), _) => Some(first),
            (None, later) => later,
        })
    })
}

/// One LLM call: each item's grams (None: no typical weight), and the model.
async fn ask(
    batch: &[(String, String)],
) -> Result<(Vec<(WeightKey, Option<f64>)>, String), AiError> {
    let (client, model) = CLIENT.as_ref().map_err(|e| AiError::Config(e.clone()))?;
    let result = estimate_ingredient_weights(client, batch).await?;
    Ok((
        result
            .weights
            .into_iter()
            .map(|((food, unit), grams)| (WeightKey { food, unit }, grams))
            .collect(),
        model.clone(),
    ))
}

async fn save_estimated(
    pool: &Arc<DbPool>,
    estimates: Vec<(WeightKey, Option<f64>)>,
    model: String,
) -> Result<(), String> {
    let estimated = estimates.len();
    run_blocking(pool, move |conn| {
        conn.transaction(|conn| {
            let now = Utc::now();
            for (key, grams) in &estimates {
                diesel::update(weights::table.find((&key.food, &key.unit)))
                    .set((
                        weights::status.eq(RESOLVED),
                        weights::grams.eq(grams),
                        weights::model.eq(&model),
                        weights::error.eq(None::<String>),
                        weights::attempts.eq(weights::attempts + 1),
                        weights::updated_at.eq(now),
                    ))
                    .execute(conn)?;
            }
            QueryResult::Ok(())
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    tracing::info!(estimated, "estimated ingredient weights");
    Ok(())
}

/// Record `error` on `items`, and whether it was the provider's.
async fn fail(
    pool: &Arc<DbPool>,
    items: Vec<(String, String)>,
    error: AiError,
) -> Result<(String, bool), String> {
    let provider_wide = !matches!(error, AiError::ParseError(_));
    let error = error.to_string();
    tracing::warn!(
        items = items.len(),
        "ingredient weight estimation failed: {}",
        error
    );
    let message = error.clone();
    let now = Utc::now();
    run_blocking(pool, move |conn| {
        conn.transaction(|conn| {
            for (food, unit) in &items {
                diesel::update(weights::table.find((food, unit)))
                    .set((
                        weights::status.eq(FAILED),
                        weights::error.eq(&message),
                        weights::attempts.eq(weights::attempts + 1),
                        weights::updated_at.eq(now),
                    ))
                    .execute(conn)?;
            }
            QueryResult::Ok(())
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    Ok((error, provider_wide))
}
