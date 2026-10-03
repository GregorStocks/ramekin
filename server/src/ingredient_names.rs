//! Catalog step 3: ingredient names the committed catalog doesn't know are
//! queued when a recipe or shopping-list item is saved, resolved by an LLM in
//! the background, and read back as a `Learned` map when estimating calories
//! or categorizing.
//!
//! Each name is a row in `ingredient_name_resolutions` carrying its own state
//! (pending, resolved, failed), so a name is resolved once however many saves
//! mention it. One worker resolves pending names in batches; a batch lock
//! keeps two callers from paying for the same names. Saves never wait on it,
//! reads never call the LLM, and a failed batch stays visible (Settings ->
//! Ingredient recognition) until retried.

use std::collections::BTreeSet;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use chrono::Utc;
use diesel::prelude::*;
use ramekin_core::ai::{
    resolve_ingredient_names, AiConfig, AiError, CachingAiClient, ConfigError, NameQuery,
    NameResolution,
};
use ramekin_core::catalog::{
    candidates, is_ambiguous, learned_key_resolves, unlearned_name, EstimatedFood, Learned,
    LearnedTarget,
};
use tokio::sync::{Mutex, Notify};

use crate::db::{run_blocking, DbPool};
use crate::schema::ingredient_name_resolutions as names;

/// Names resolved per LLM call.
pub(crate) const BATCH_SIZE: i64 = 40;
/// Candidate catalog keys offered per name.
const CANDIDATES: usize = 12;
/// How soon the worker tries again after a pass that failed, doubling up to
/// `RETRY_MAX` while failures continue.
const RETRY_MIN: Duration = Duration::from_secs(60);
const RETRY_MAX: Duration = Duration::from_secs(60 * 60);

pub const PENDING: &str = "pending";
pub const RESOLVED: &str = "resolved";
pub const FAILED: &str = "failed";

static WAKE: LazyLock<Notify> = LazyLock::new(Notify::new);
pub(crate) static BATCH: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
/// One client for the process, so its rate limit spaces every call, with
/// the model it asks.
pub(crate) static CLIENT: LazyLock<Result<(CachingAiClient, String), ConfigError>> =
    LazyLock::new(|| {
        AiConfig::from_env().map(|config| {
            let model = config.model.clone();
            (CachingAiClient::new(config), model)
        })
    });

/// The names among `items` the committed catalog doesn't know, deduplicated.
pub fn unlearned_names<'a>(items: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    items
        .into_iter()
        .filter_map(unlearned_name)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Queue names for resolution. Names already queued or resolved are left
/// alone, so a race between saves can never fail either.
pub fn enqueue(conn: &mut PgConnection, new_names: &[String]) -> QueryResult<usize> {
    if new_names.is_empty() {
        return Ok(0);
    }
    let rows: Vec<_> = new_names
        .iter()
        .map(|name| (names::name.eq(name), names::status.eq(PENDING)))
        .collect();
    diesel::insert_into(names::table)
        .values(&rows)
        .on_conflict_do_nothing()
        .execute(conn)
}

/// Put failed names back in the queue, clearing their error, so the next
/// pass asks about them again.
pub fn requeue_failed(conn: &mut PgConnection, failed: &[String]) -> QueryResult<usize> {
    diesel::update(
        names::table
            .filter(names::status.eq(FAILED))
            .filter(names::name.eq_any(failed)),
    )
    .set((names::status.eq(PENDING), names::error.eq(None::<String>)))
    .execute(conn)
}

/// Queue the unknown names in `items`. Write paths call this inside their
/// transaction, so a save and its queued names commit together, then call
/// `wake` once it has committed.
pub fn enqueue_items<'a>(
    conn: &mut PgConnection,
    items: impl IntoIterator<Item = &'a str>,
) -> QueryResult<usize> {
    enqueue(conn, &unlearned_names(items))
}

/// The most new names one read queues: a recipe rarely has more, and the
/// rest are queued on later reads, so a read can't grow the paid queue
/// without bound.
const MAX_QUEUED_PER_READ: usize = 50;

/// Queue `items`' unknown or ambiguous names that have no row yet, up to
/// `MAX_QUEUED_PER_READ`. A read path calls this, so recipes saved before a
/// catalog change (or before ambiguous names were learned) still get their
/// names resolved. Returns whether any were left for a later read, which
/// should keep the reader polling like a pending name.
pub fn enqueue_unasked<'a>(
    conn: &mut PgConnection,
    items: impl IntoIterator<Item = &'a str>,
) -> QueryResult<bool> {
    let wanted = unlearned_names(items);
    if wanted.is_empty() {
        return Ok(false);
    }
    let asked: Vec<String> = names::table
        .filter(names::name.eq_any(&wanted))
        .select(names::name)
        .load(conn)?;
    let unasked: Vec<String> = wanted
        .into_iter()
        .filter(|name| !asked.contains(name))
        .collect();
    let (now, later) = unasked.split_at(unasked.len().min(MAX_QUEUED_PER_READ));
    enqueue(conn, now)?;
    Ok(!later.is_empty())
}

/// Wake the worker.
pub fn wake() {
    WAKE.notify_one();
}

/// Resolved answers for `items`' unknown names, for estimating or
/// categorizing. Pending, failed, and never-queued names are simply absent,
/// so they read as unknown.
pub fn load_learned<'a>(
    conn: &mut PgConnection,
    items: impl IntoIterator<Item = &'a str>,
) -> QueryResult<Learned> {
    let wanted = unlearned_names(items);
    if wanted.is_empty() {
        return Ok(Learned::new());
    }
    let rows: Vec<LearnedRow> = names::table
        .filter(names::name.eq_any(&wanted))
        .filter(names::status.eq(RESOLVED))
        .select((
            names::name,
            names::disposition,
            names::catalog_key,
            names::kcal_per_100g,
            names::grams_per_cup,
            names::grams_per_piece,
        ))
        .load(conn)?;
    Ok(rows
        .into_iter()
        .map(|(name, disposition, key, kcal, cup, piece)| {
            let target = match (disposition.as_deref(), key, kcal) {
                (Some("entry"), Some(key), _) => LearnedTarget::Entry(key),
                (Some("estimate"), _, Some(kcal_per_100g)) => {
                    LearnedTarget::Estimate(EstimatedFood {
                        kcal_per_100g,
                        grams_per_cup: cup,
                        grams_per_piece: piece,
                    })
                }
                (Some("not_food"), _, _) => LearnedTarget::NotFood,
                _ => LearnedTarget::Unknown,
            };
            (name, target)
        })
        .collect())
}

/// Name, disposition, key, and an estimate's calories, cup and piece weights.
type LearnedRow = (
    String,
    Option<String>,
    Option<String>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
);

/// Whether any of `items`' unknown names is still waiting to be resolved,
/// so a reader knows a fresher answer is coming.
pub fn any_pending<'a>(
    conn: &mut PgConnection,
    items: impl IntoIterator<Item = &'a str>,
) -> QueryResult<bool> {
    let wanted = unlearned_names(items);
    if wanted.is_empty() {
        return Ok(false);
    }
    diesel::select(diesel::dsl::exists(
        names::table
            .filter(names::name.eq_any(&wanted))
            .filter(names::status.eq(PENDING)),
    ))
    .get_result(conn)
}

/// Requeue learned answers whose catalog key no longer names one entry (a
/// catalog change removed or split it), so they're asked about again rather
/// than silently reading as unknown. Runs at startup, before serving, since
/// the catalog only changes with a deploy.
pub fn requeue_stale_keys(pool: &DbPool) -> Result<usize, String> {
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    let keys: Vec<(String, String)> = names::table
        .filter(names::status.eq(RESOLVED))
        .filter(names::catalog_key.is_not_null())
        .select((names::name, names::catalog_key.assume_not_null()))
        .load(&mut conn)
        .map_err(|e| e.to_string())?;
    let stale: Vec<String> = keys
        .into_iter()
        .filter(|(_, key)| !learned_key_resolves(key))
        .map(|(name, _)| name)
        .collect();
    if stale.is_empty() {
        return Ok(0);
    }
    conn.transaction(|conn| {
        let now = Utc::now();
        // Their items' computed category changes now, so incremental sync
        // must send them again.
        for name in &stale {
            touch_shopping_items(conn, name, now)?;
        }
        diesel::update(names::table.filter(names::name.eq_any(&stale)))
            .set((
                names::status.eq(PENDING),
                names::disposition.eq(None::<String>),
                names::catalog_key.eq(None::<String>),
                names::updated_at.eq(now),
            ))
            .execute(conn)
    })
    .map_err(|e: diesel::result::Error| e.to_string())
}

/// Start the worker. It first drains whatever was pending at startup, then
/// waits to be woken.
pub fn spawn_worker(pool: Arc<DbPool>) {
    tokio::spawn(async move {
        let mut backoff = RETRY_MIN;
        loop {
            match work_pass(&pool).await {
                Ok(()) => {
                    backoff = RETRY_MIN;
                    WAKE.notified().await;
                }
                // Names may still be pending (a provider outage, or a
                // database error), so try again later even if nothing wakes us.
                Err(e) => {
                    tracing::warn!(
                        retry_in_secs = backoff.as_secs(),
                        "Ingredient name resolution failed: {}",
                        e
                    );
                    tokio::select! {
                        () = WAKE.notified() => {}
                        () = tokio::time::sleep(backoff) => {}
                    }
                    backoff = (backoff * 2).min(RETRY_MAX);
                }
            }
        }
    });
}

/// How taking one batch from a queue went. Failures are already recorded on
/// their rows.
pub(crate) enum Step {
    /// Nothing was pending.
    Idle,
    Done,
    /// The model's answer for some rows was invalid.
    Failed(String),
    /// The provider or configuration failed, so later batches would too.
    ProviderFailed(String),
}

/// Work both queues until both are empty, a batch from each in turn, so a
/// backlog of names never starves weights or the reverse. Names go first in
/// each turn: an answer can make a line's food known, and so its weight worth
/// asking for. Returns the first failure.
async fn work_pass(pool: &Arc<DbPool>) -> Result<(), String> {
    let mut first_error = None;
    // A queue whose provider failed sits out the rest of the pass: its next
    // batch would fail the same way, so it waits for the worker's retry. An
    // idle queue is checked again every turn, since saves keep adding work.
    let (mut names_down, mut weights_down) = (false, false);
    loop {
        let mut worked = false;
        if !names_down {
            let step = resolve_next_batch(pool, None).await?;
            names_down = matches!(step, Step::ProviderFailed(_));
            worked |= record(step, &mut first_error);
        }
        if !weights_down {
            let step = crate::ingredient_weights::estimate_next_batch(pool).await?;
            weights_down = matches!(step, Step::ProviderFailed(_));
            worked |= record(step, &mut first_error);
        }
        if !worked {
            break;
        }
    }
    first_error.map_or(Ok(()), Err)
}

/// Keep `step`'s error, if it's the first, and say whether it took a batch
/// that a queue may have more of.
fn record(step: Step, first_error: &mut Option<String>) -> bool {
    match step {
        Step::Idle => false,
        Step::Done => true,
        Step::Failed(e) => {
            first_error.get_or_insert(e);
            true
        }
        Step::ProviderFailed(e) => {
            first_error.get_or_insert(e);
            false
        }
    }
}

/// Resolve one batch of pending names, or only of `only`. The batch is taken
/// under the batch lock, so no name is asked about twice, and one at a time,
/// so a scrape job's few names wait for at most the batch in flight.
async fn resolve_next_batch(pool: &Arc<DbPool>, only: Option<Vec<String>>) -> Result<Step, String> {
    let _batch = BATCH.lock().await;
    let batch: Vec<String> = run_blocking(pool, move |conn| {
        let mut query = names::table
            .filter(names::status.eq(PENDING))
            .select(names::name)
            .order(names::name)
            .limit(BATCH_SIZE)
            .into_boxed();
        if let Some(only) = only {
            query = query.filter(names::name.eq_any(only));
        }
        query.load(conn)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    if batch.is_empty() {
        return Ok(Step::Idle);
    }
    // A failed answer is recorded on its names; failing to record it would
    // leave them pending, so `Err` stops rather than asking again.
    Ok(match resolve_batch(pool, batch).await? {
        Batch::Resolved => Step::Done,
        Batch::NamesFailed(e) => Step::Failed(e),
        Batch::ProviderFailed(e) => Step::ProviderFailed(e),
    })
}

/// Resolve pending names among `only` in batches (a scrape job's names).
/// Returns the first batch error, or a failure another caller already
/// recorded on one of them.
pub async fn resolve_pending(pool: &Arc<DbPool>, only: Vec<String>) -> Result<(), String> {
    let mut first_error = None;
    while record(
        resolve_next_batch(pool, Some(only.clone())).await?,
        &mut first_error,
    ) {}
    if let Some(e) = first_error {
        return Err(e);
    }
    let failed: Vec<(String, Option<String>)> = run_blocking(pool, move |conn| {
        names::table
            .filter(names::name.eq_any(only))
            .filter(names::status.eq(FAILED))
            .select((names::name, names::error))
            .load(conn)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    match failed.into_iter().next() {
        Some((name, error)) => Err(format!(
            "{name:?}: {}",
            error.unwrap_or_else(|| "resolution failed".to_string())
        )),
        None => Ok(()),
    }
}

/// How a batch ended. Its failures are already recorded on its names.
enum Batch {
    Resolved,
    /// The model's answer for some names was invalid.
    NamesFailed(String),
    /// The provider or configuration failed, so later batches would too.
    ProviderFailed(String),
}

/// Resolve one batch. If the model's answer for a multi-name batch is
/// invalid, each name is retried alone, so one name the model can't handle
/// doesn't fail the others; only the names that fail on their own are marked
/// failed. Provider and configuration errors fail the batch at once. `Err`
/// means an outcome couldn't be saved.
async fn resolve_batch(pool: &Arc<DbPool>, batch: Vec<String>) -> Result<Batch, String> {
    let error = match ask(&batch).await {
        Ok((resolutions, model)) => {
            save_resolved(pool, resolutions, model).await?;
            return Ok(Batch::Resolved);
        }
        Err(AiError::ParseError(error)) if batch.len() > 1 => error,
        Err(error) => return fail(pool, batch, error).await,
    };
    tracing::warn!(
        names = batch.len(),
        "ingredient name batch failed, retrying names one at a time: {}",
        error
    );
    let mut outcome = Batch::Resolved;
    for name in batch {
        let single = vec![name];
        match ask(&single).await {
            Ok((resolutions, model)) => save_resolved(pool, resolutions, model).await?,
            Err(error) => match fail(pool, single, error).await? {
                Batch::NamesFailed(e) => {
                    if matches!(outcome, Batch::Resolved) {
                        outcome = Batch::NamesFailed(e);
                    }
                }
                // Names not yet asked about stay pending.
                provider => return Ok(provider),
            },
        }
    }
    Ok(outcome)
}

/// Record `error` on `names` and classify it.
async fn fail(pool: &Arc<DbPool>, names: Vec<String>, error: AiError) -> Result<Batch, String> {
    let provider_wide = !matches!(error, AiError::ParseError(_));
    let error = error.to_string();
    save_failed(pool, names, error.clone()).await?;
    Ok(if provider_wide {
        Batch::ProviderFailed(error)
    } else {
        Batch::NamesFailed(error)
    })
}

/// One LLM call for `batch`: each name's answer, and the model that gave it.
async fn ask(batch: &[String]) -> Result<(Vec<(String, NameResolution)>, String), AiError> {
    let prompt: Vec<NameQuery> = batch
        .iter()
        .map(|name| NameQuery {
            name: name.clone(),
            candidates: candidates(name, CANDIDATES)
                .into_iter()
                .map(str::to_string)
                .collect(),
            ambiguous: is_ambiguous(name),
        })
        .collect();
    let (client, model) = CLIENT.as_ref().map_err(|e| AiError::Config(e.clone()))?;
    let result = resolve_ingredient_names(client, &prompt).await?;
    Ok((result.resolutions.into_iter().collect(), model.clone()))
}

async fn save_resolved(
    pool: &Arc<DbPool>,
    resolutions: Vec<(String, NameResolution)>,
    model: String,
) -> Result<(), String> {
    let resolved = resolutions.len();
    run_blocking(pool, move |conn| {
        conn.transaction(|conn| {
            // Taken once the connection is ours, to keep the touch as close
            // to its commit as possible for incremental sync.
            let now = Utc::now();
            for (name, resolution) in &resolutions {
                let (disposition, key, estimate) = match resolution {
                    NameResolution::Entry(key) => ("entry", Some(key.as_str()), None),
                    NameResolution::Estimate(estimate) => ("estimate", None, Some(estimate)),
                    NameResolution::NotFood => ("not_food", None, None),
                    NameResolution::Unknown => ("unknown", None, None),
                };
                // Only entries and non-foods can change a shopping category.
                if matches!(
                    resolution,
                    NameResolution::Entry(_) | NameResolution::NotFood
                ) {
                    touch_shopping_items(conn, name, now)?;
                }
                diesel::update(names::table.find(name))
                    .set((
                        names::status.eq(RESOLVED),
                        names::disposition.eq(disposition),
                        names::catalog_key.eq(key),
                        names::kcal_per_100g.eq(estimate.map(|e| e.kcal_per_100g)),
                        names::grams_per_cup.eq(estimate.and_then(|e| e.grams_per_cup)),
                        names::grams_per_piece.eq(estimate.and_then(|e| e.grams_per_piece)),
                        names::model.eq(&model),
                        names::error.eq(None::<String>),
                        names::attempts.eq(names::attempts + 1),
                        names::updated_at.eq(now),
                    ))
                    .execute(conn)?;
            }
            QueryResult::Ok(())
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    tracing::info!(resolved, "resolved ingredient names");
    Ok(())
}

/// Mark shopping-list items with this name as changed, so incremental sync
/// sends their new computed category. Only `updated_at` moves: the item
/// itself is unchanged, so its version (and clients' pending edits) stay valid.
fn touch_shopping_items(
    conn: &mut PgConnection,
    name: &str,
    now: chrono::DateTime<Utc>,
) -> QueryResult<()> {
    use crate::schema::shopping_list_items as items;
    // Case-insensitive prefilter; the exact match is the catalog normalization.
    let pattern = name
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
        .replace(' ', "%");
    // Unanchored, so spellings with surrounding text or whitespace that
    // normalize to `name` still reach the exact check below.
    let pattern = format!("%{pattern}%");
    let candidates: Vec<(uuid::Uuid, String)> = items::table
        .filter(items::deleted_at.is_null())
        .filter(items::item.ilike(pattern).escape('\\'))
        .select((items::id, items::item))
        .load(conn)?;
    let ids: Vec<uuid::Uuid> = candidates
        .into_iter()
        .filter(|(_, item)| unlearned_name(item).as_deref() == Some(name))
        .map(|(id, _)| id)
        .collect();
    if !ids.is_empty() {
        diesel::update(items::table.filter(items::id.eq_any(ids)))
            .set(items::updated_at.eq(now))
            .execute(conn)?;
    }
    Ok(())
}

async fn save_failed(pool: &Arc<DbPool>, failed: Vec<String>, error: String) -> Result<(), String> {
    tracing::warn!(
        names = failed.len(),
        "ingredient name resolution failed: {}",
        error
    );
    let now = Utc::now();
    run_blocking(pool, move |conn| {
        diesel::update(names::table.filter(names::name.eq_any(&failed)))
            .set((
                names::status.eq(FAILED),
                names::error.eq(&error),
                names::attempts.eq(names::attempts + 1),
                names::updated_at.eq(now),
            ))
            .execute(conn)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    Ok(())
}
