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

use chrono::Utc;
use diesel::prelude::*;
use ramekin_core::ai::{resolve_ingredient_names, AiConfig, CachingAiClient, NameResolution};
use ramekin_core::catalog::{candidates, unlearned_name, Learned, LearnedTarget};
use tokio::sync::{Mutex, Notify};

use crate::db::{run_blocking, DbPool};
use crate::schema::ingredient_name_resolutions as names;

/// Names resolved per LLM call.
const BATCH_SIZE: i64 = 40;
/// Candidate catalog keys offered per name.
const CANDIDATES: usize = 12;

pub const PENDING: &str = "pending";
pub const RESOLVED: &str = "resolved";
pub const FAILED: &str = "failed";

static WAKE: LazyLock<Notify> = LazyLock::new(Notify::new);
static BATCH: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

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
    let rows: Vec<(String, Option<String>, Option<String>)> = names::table
        .filter(names::name.eq_any(&wanted))
        .filter(names::status.eq(RESOLVED))
        .select((names::name, names::disposition, names::catalog_key))
        .load(conn)?;
    Ok(rows
        .into_iter()
        .map(|(name, disposition, key)| {
            let target = match (disposition.as_deref(), key) {
                (Some("entry"), Some(key)) => LearnedTarget::Entry(key),
                (Some("not_food"), _) => LearnedTarget::NotFood,
                _ => LearnedTarget::Unknown,
            };
            (name, target)
        })
        .collect())
}

/// Start the worker. It first drains whatever was pending at startup, then
/// waits to be woken.
pub fn spawn_worker(pool: Arc<DbPool>) {
    tokio::spawn(async move {
        loop {
            if let Err(e) = resolve_pending(&pool, None).await {
                tracing::warn!("Ingredient name resolution failed: {}", e);
            }
            WAKE.notified().await;
        }
    });
}

/// Resolve pending names, all of them or only `only`, in batches. Returns
/// the first batch error; that batch's names are marked failed with it.
pub async fn resolve_pending(pool: &Arc<DbPool>, only: Option<Vec<String>>) -> Result<(), String> {
    let mut first_error = None;
    loop {
        // One batch at a time, taken fairly, so a scrape job's few names wait
        // for at most the batch in flight rather than a whole backlog. The
        // batch is chosen under the lock, so no name is asked about twice.
        let _batch = BATCH.lock().await;
        let filter = only.clone();
        let batch: Vec<String> = run_blocking(pool, move |conn| {
            let mut query = names::table
                .filter(names::status.eq(PENDING))
                .select(names::name)
                .order(names::name)
                .limit(BATCH_SIZE)
                .into_boxed();
            if let Some(only) = filter {
                query = query.filter(names::name.eq_any(only));
            }
            query.load(conn)
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
        if batch.is_empty() {
            break;
        }
        if let Err(e) = resolve_batch(pool, batch).await {
            first_error.get_or_insert(e);
        }
    }
    match (first_error, only) {
        (Some(e), _) => Err(e),
        // Names another caller already failed are still failures here.
        (None, Some(only)) => {
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
        (None, None) => Ok(()),
    }
}

/// Resolve one batch. If a multi-name batch fails, each name is retried
/// alone, so one name the model can't handle doesn't fail the others; only
/// the names that fail on their own are marked failed.
async fn resolve_batch(pool: &Arc<DbPool>, batch: Vec<String>) -> Result<(), String> {
    match ask(&batch).await {
        Ok((resolutions, model)) => save_resolved(pool, resolutions, model).await,
        Err(error) if batch.len() == 1 => {
            save_failed(pool, batch, error.clone()).await?;
            Err(error)
        }
        Err(error) => {
            tracing::warn!(
                names = batch.len(),
                "ingredient name batch failed, retrying names one at a time: {}",
                error
            );
            let mut first_error = None;
            for name in batch {
                let single = vec![name];
                let outcome = match ask(&single).await {
                    Ok((resolutions, model)) => save_resolved(pool, resolutions, model).await,
                    Err(error) => {
                        save_failed(pool, single, error.clone()).await?;
                        Err(error)
                    }
                };
                if let Err(e) = outcome {
                    first_error.get_or_insert(e);
                }
            }
            first_error.map_or(Ok(()), Err)
        }
    }
}

/// One LLM call for `batch`: each name's answer, and the model that gave it.
async fn ask(batch: &[String]) -> Result<(Vec<(String, NameResolution)>, String), String> {
    let prompt: Vec<(String, Vec<String>)> = batch
        .iter()
        .map(|name| {
            let keys = candidates(name, CANDIDATES)
                .into_iter()
                .map(str::to_string)
                .collect();
            (name.clone(), keys)
        })
        .collect();
    let config = AiConfig::from_env().map_err(|e| format!("AI service unavailable: {e}"))?;
    let model = config.model.clone();
    let result = resolve_ingredient_names(&CachingAiClient::new(config), &prompt)
        .await
        .map_err(|e| e.to_string())?;
    Ok((result.resolutions.into_iter().collect(), model))
}

async fn save_resolved(
    pool: &Arc<DbPool>,
    resolutions: Vec<(String, NameResolution)>,
    model: String,
) -> Result<(), String> {
    let now = Utc::now();
    let resolved = resolutions.len();
    run_blocking(pool, move |conn| {
        conn.transaction(|conn| {
            for (name, resolution) in &resolutions {
                let (disposition, key) = match resolution {
                    NameResolution::Entry(key) => ("entry", Some(key.as_str())),
                    NameResolution::NotFood => ("not_food", None),
                    NameResolution::Unknown => ("unknown", None),
                };
                if !matches!(resolution, NameResolution::Unknown) {
                    touch_shopping_items(conn, name, now)?;
                }
                diesel::update(names::table.find(name))
                    .set((
                        names::status.eq(RESOLVED),
                        names::disposition.eq(disposition),
                        names::catalog_key.eq(key),
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
