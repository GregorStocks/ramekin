//! Catalog step 3: ingredient names the committed catalog doesn't know are
//! queued when a recipe or shopping-list item is saved, resolved by an LLM in
//! the background, and read back as a `Learned` map when estimating calories
//! or categorizing.
//!
//! Each name is a row in `ingredient_name_resolutions` carrying its own state
//! (pending, resolved, failed), so a name is resolved once however many saves
//! mention it. One worker resolves pending names in batches; a batch lock
//! keeps two callers from paying for the same names. Saves never wait on it,
//! and reads neither call the LLM nor queue names. An invalid answer is asked
//! again up to `MAX_ATTEMPTS` times; a name that still fails stays visible
//! (Settings -> Ingredient recognition) until retried. An answer is kept until
//! the catalog stops accepting it (see `CatalogSync`): names are never re-asked
//! because the model or the candidate list changed.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use chrono::Utc;
use diesel::prelude::*;
use ramekin_core::ai::{
    resolve_ingredient_names, AiConfig, AiError, CachingAiClient, ConfigError, NameQuery,
    NameResolution,
};
use ramekin_core::catalog::{
    candidates, is_ambiguous, learned_key_resolves, normalize, unlearned_name, EstimatedFood,
    Learned, LearnedTarget,
};
use tokio::sync::{Mutex, Notify};

use crate::db::{run_blocking, DbPool};
use crate::schema::ingredient_name_resolutions as names;

/// Names resolved per LLM call.
const BATCH_SIZE: i64 = 40;
/// Candidate catalog keys offered per name.
const CANDIDATES: usize = 12;
/// How soon the worker tries again after a pass that failed, doubling up to
/// `RETRY_MAX` while failures continue.
const RETRY_MIN: Duration = Duration::from_secs(60);
const RETRY_MAX: Duration = Duration::from_secs(60 * 60);

pub const PENDING: &str = "pending";
pub const RESOLVED: &str = "resolved";
pub const FAILED: &str = "failed";
/// How many invalid answers a name gets before it's marked failed.
const MAX_ATTEMPTS: i32 = 3;

static WAKE: LazyLock<Notify> = LazyLock::new(Notify::new);
/// Set whenever names are queued or put back in the queue, or the provider
/// answers. Every save wakes the worker, whether or not it queued anything,
/// so only these cut a failure backoff short; otherwise a provider outage
/// would be retried at the rate of saves.
static NEW_WORK: AtomicBool = AtomicBool::new(false);
static BATCH: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
/// One client for the process, so its rate limit spaces every call, with
/// the model it asks: the ingredient model (`AiConfig::for_ingredients`).
static CLIENT: LazyLock<Result<(CachingAiClient, String), ConfigError>> = LazyLock::new(|| {
    AiConfig::from_env().map(|config| {
        let config = config.for_ingredients();
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
    let queued = diesel::insert_into(names::table)
        .values(&rows)
        .on_conflict_do_nothing()
        .execute(conn)?;
    note_new_work(queued);
    Ok(queued)
}

/// Put failed names back in the queue with a fresh set of attempts.
pub fn requeue_failed(conn: &mut PgConnection, failed: &[String]) -> QueryResult<usize> {
    let pending = diesel::update(
        names::table
            .filter(names::status.eq(FAILED))
            .filter(names::name.eq_any(failed)),
    )
    .set((
        names::status.eq(PENDING),
        names::error.eq(None::<String>),
        names::attempts.eq(0),
    ))
    .execute(conn)?;
    note_new_work(pending);
    Ok(pending)
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

/// Record that `queued` names were added to the queue.
fn note_new_work(queued: usize) {
    if queued > 0 {
        NEW_WORK.store(true, Ordering::SeqCst);
    }
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

/// `load_learned` for a write that stores something computed from the answers
/// (a shopping item's category), after queueing the names it wrote. The name
/// rows are locked until the write commits, so the worker can't resolve one
/// in between: its update waits for the write, and its
/// `recategorize_shopping_items` then sees the written item. Queueing first
/// gives every unknown name a row to lock: an insert racing another write's
/// insert of the same new name waits for it to commit.
pub fn load_learned_for_write<'a>(
    conn: &mut PgConnection,
    items: impl IntoIterator<Item = &'a str>,
) -> QueryResult<Learned> {
    let items: Vec<&str> = items.into_iter().collect();
    let wanted = unlearned_names(items.iter().copied());
    if !wanted.is_empty() {
        names::table
            .filter(names::name.eq_any(&wanted))
            .select(names::name)
            .for_share()
            .load::<String>(conn)?;
    }
    load_learned(conn, items)
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

/// Why the startup sync puts a resolved name back in the queue: its answer
/// no longer fits the committed catalog. Nothing else re-asks a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CatalogSync {
    /// The learned key no longer names one entry (removed or split): ask
    /// again rather than silently reading as unknown.
    StaleKey,
    /// The catalog now calls the name ambiguous, which rules out an estimate
    /// or a non-food answer: ask again for the food a recipe most likely means.
    NowAmbiguous,
}

/// A stored name as the startup sync reads it.
#[derive(Debug, Queryable)]
pub struct StoredName {
    pub name: String,
    pub status: String,
    pub disposition: Option<String>,
    pub catalog_key: Option<String>,
}

/// Whether the committed catalog rules out a stored answer. A name the
/// catalog knows itself is never looked up here, so its row is left alone.
pub fn catalog_sync(row: &StoredName) -> Option<CatalogSync> {
    if row.status != RESOLVED || unlearned_name(&row.name).is_none() {
        return None;
    }
    match (row.disposition.as_deref(), row.catalog_key.as_deref()) {
        (Some("entry"), Some(key)) if !learned_key_resolves(key) => Some(CatalogSync::StaleKey),
        (Some("estimate" | "not_food"), _) if is_ambiguous(&row.name) => {
            Some(CatalogSync::NowAmbiguous)
        }
        _ => None,
    }
}

/// The candidate keys the model is offered for `name`.
fn offered(name: &str) -> Vec<String> {
    candidates(name, CANDIDATES)
        .into_iter()
        .map(str::to_string)
        .collect()
}

/// Put names whose answer the committed catalog rules out (see
/// `CatalogSync`) back in the queue, returning how many of each. Runs at
/// startup, before serving, since the catalog only changes with a deploy.
pub fn sync_with_catalog(pool: &DbPool) -> Result<BTreeMap<CatalogSync, usize>, String> {
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    let rows: Vec<StoredName> = names::table
        .filter(names::status.eq(RESOLVED))
        .select((
            names::name,
            names::status,
            names::disposition,
            names::catalog_key,
        ))
        .load(&mut conn)
        .map_err(|e| e.to_string())?;
    let mut changes: BTreeMap<CatalogSync, Vec<String>> = BTreeMap::new();
    for row in rows {
        if let Some(sync) = catalog_sync(&row) {
            changes.entry(sync).or_default().push(row.name);
        }
    }
    let requeued: Vec<&String> = changes.values().flatten().collect();
    if !requeued.is_empty() {
        conn.transaction(|conn| {
            let now = Utc::now();
            diesel::update(names::table.filter(names::name.eq_any(&requeued)))
                .set((
                    names::status.eq(PENDING),
                    names::disposition.eq(None::<String>),
                    names::catalog_key.eq(None::<String>),
                    names::kcal_per_100g.eq(None::<f64>),
                    names::grams_per_cup.eq(None::<f64>),
                    names::grams_per_piece.eq(None::<f64>),
                    names::error.eq(None::<String>),
                    names::attempts.eq(0),
                    names::updated_at.eq(now),
                ))
                .execute(conn)?;
            QueryResult::Ok(())
        })
        .map_err(|e| e.to_string())?;
        // The old answer may have given these items their category.
        let names: HashSet<String> = requeued.iter().map(|name| name.to_string()).collect();
        recategorize_shopping_items(&mut conn, &names).map_err(|e| e.to_string())?;
    }
    Ok(changes
        .into_iter()
        .map(|(sync, changed)| (sync, changed.len()))
        .collect())
}

/// Start the worker. It first drains whatever was pending at startup, then
/// waits to be woken.
pub fn spawn_worker(pool: Arc<DbPool>) {
    tokio::spawn(async move {
        let mut backoff = RETRY_MIN;
        loop {
            NEW_WORK.store(false, Ordering::SeqCst);
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
                    let retry_at = tokio::time::Instant::now() + backoff;
                    loop {
                        tokio::select! {
                            () = WAKE.notified() => if NEW_WORK.swap(false, Ordering::SeqCst) {
                                break;
                            },
                            () = tokio::time::sleep_until(retry_at) => break,
                        }
                    }
                    backoff = (backoff * 2).min(RETRY_MAX);
                }
            }
        }
    });
}

/// Queue the unknown or ambiguous names of every stored recipe and
/// shopping-list item, returning how many were new. Saves queue their own
/// names, so this only finds names a catalog change stopped recognizing; the
/// server runs it at startup, since the catalog only changes with a deploy.
pub fn queue_stored(conn: &mut PgConnection) -> Result<usize, String> {
    use crate::models::Ingredient;
    use crate::schema::{recipe_versions, recipes, shopping_list_items};

    let versions: Vec<serde_json::Value> = recipes::table
        .inner_join(
            recipe_versions::table
                .on(recipes::current_version_id.eq(recipe_versions::id.nullable())),
        )
        .filter(recipes::deleted_at.is_null())
        .select(recipe_versions::ingredients)
        .load(conn)
        .map_err(|e| e.to_string())?;
    let mut items: Vec<String> = Vec::new();
    for ingredients in versions {
        let ingredients = serde_json::from_value::<Vec<Ingredient>>(ingredients)
            .map_err(|e| format!("stored recipe ingredients are not valid: {e}"))?;
        items.extend(ingredients.into_iter().map(|ingredient| ingredient.item));
    }
    let shopping: Vec<String> = shopping_list_items::table
        .filter(shopping_list_items::deleted_at.is_null())
        .select(shopping_list_items::item)
        .load(conn)
        .map_err(|e| e.to_string())?;
    items.extend(shopping);
    enqueue(conn, &unlearned_names(items.iter().map(String::as_str))).map_err(|e| e.to_string())
}

/// How taking one batch from a queue went. Failures are already recorded on
/// their rows.
enum Step {
    /// Nothing was pending.
    Idle,
    Done,
    /// The model's answer for some rows was invalid.
    Failed(String),
    /// The provider or configuration failed, so later batches would too.
    ProviderFailed(String),
}

/// Resolve pending names until none are left or the provider fails. Returns
/// the first failure.
async fn work_pass(pool: &Arc<DbPool>) -> Result<(), String> {
    let mut first_error = None;
    while record(resolve_next_batch(pool, None).await?, &mut first_error) {}
    first_error.map_or(Ok(()), Err)
}

/// Keep `step`'s error, if it's the first, and say whether it took a batch
/// that the queue may have more of.
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
    /// Answered, or an invalid answer left names pending for another attempt.
    Resolved,
    /// The model's answers for some names stayed invalid until they ran out
    /// of attempts.
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
        Err(error) if error.is_answer_specific() && batch.len() > 1 => error.to_string(),
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
    let provider_wide = !error.is_answer_specific();
    let error = error.to_string();
    let given_up = save_failed(pool, names, error.clone(), provider_wide).await?;
    Ok(if provider_wide {
        // The names stay pending. A scrape job resolving its own names
        // directly may have hit this while the worker was idle, so wake it to
        // retry them with its backoff.
        wake();
        Batch::ProviderFailed(error)
    } else if given_up > 0 {
        Batch::NamesFailed(error)
    } else {
        // Still pending with attempts left: not a failure yet, and the next
        // batch asks again.
        Batch::Resolved
    })
}

/// One LLM call for `batch`: each name's answer, and the model that gave it.
async fn ask(batch: &[String]) -> Result<(Vec<(String, NameResolution)>, String), AiError> {
    let prompt: Vec<NameQuery> = batch
        .iter()
        .map(|name| NameQuery {
            name: name.clone(),
            candidates: offered(name),
            ambiguous: is_ambiguous(name),
        })
        .collect();
    let (client, model) = CLIENT.as_ref().map_err(|e| AiError::Config(e.clone()))?;
    let mut result = resolve_ingredient_names(client, &prompt).await?;
    let answers = prompt
        .into_iter()
        .filter_map(|query| {
            let resolution = result.resolutions.remove(&query.name)?;
            Some((query.name, resolution))
        })
        .collect();
    Ok((answers, model.clone()))
}

async fn save_resolved(
    pool: &Arc<DbPool>,
    answers: Vec<(String, NameResolution)>,
    model: String,
) -> Result<(), String> {
    let resolved = answers.len();
    // Only entries and non-foods give a shopping category. A pending name has
    // no earlier answer, so nothing else changes one.
    let recategorized: HashSet<String> = answers
        .iter()
        .filter(|(_, resolution)| {
            matches!(
                resolution,
                NameResolution::Entry(_) | NameResolution::NotFood
            )
        })
        .map(|(name, _)| name.clone())
        .collect();
    run_blocking(pool, move |conn| {
        conn.transaction(|conn| {
            let now = Utc::now();
            for (name, resolution) in &answers {
                let (disposition, key, estimate) = match resolution {
                    NameResolution::Entry(key) => ("entry", Some(key.as_str()), None),
                    NameResolution::Estimate(estimate) => ("estimate", None, Some(estimate)),
                    NameResolution::NotFood => ("not_food", None, None),
                    NameResolution::Unknown => ("unknown", None, None),
                };
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
    run_blocking(pool, move |conn| {
        recategorize_shopping_items(conn, &recategorized)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    tracing::info!(resolved, "resolved ingredient names");
    // The provider answered, so a worker backing off from an outage should
    // retry its pending names now.
    note_new_work(resolved);
    wake();
    Ok(())
}

/// Recompute the stored category of live shopping-list items with any of
/// these names, once their new answers have committed. An item whose category
/// changed gets a new `updated_at` too, so incremental sync sends it; its
/// version (and clients' pending edits) stay valid, since the item itself is
/// unchanged.
///
/// Its own transaction, after the answers' one, so locks are always taken
/// item first, then name row, as writes take them: the matched items are
/// locked and re-read (a concurrent rename may have changed the name), then
/// the answers read under `load_learned_for_write`. A write that read an old
/// answer held that name row until it committed, so it committed before the
/// answer did, and this sees it; a later write reads the new answer itself.
/// Names match by catalog normalization, which no SQL pattern expresses, so
/// the live items are read once and matched here.
fn recategorize_shopping_items(
    conn: &mut PgConnection,
    changed: &HashSet<String>,
) -> QueryResult<()> {
    use crate::schema::shopping_list_items as items;
    if changed.is_empty() {
        return Ok(());
    }
    let matches = |item: &str| changed.contains(normalize(item).as_str());
    conn.transaction(|conn| {
        let ids: Vec<uuid::Uuid> = items::table
            .filter(items::deleted_at.is_null())
            .select((items::id, items::item))
            .load::<(uuid::Uuid, String)>(conn)?
            .into_iter()
            .filter(|(_, item)| matches(item))
            .map(|(id, _)| id)
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        let locked: Vec<(uuid::Uuid, String, Option<String>)> = items::table
            .filter(items::id.eq_any(&ids))
            .filter(items::deleted_at.is_null())
            .order(items::id)
            .select((items::id, items::item, items::computed_category))
            .for_update()
            .load(conn)?;
        let matched: Vec<_> = locked
            .into_iter()
            .filter(|(_, item, _)| matches(item))
            .collect();
        let names: Vec<&str> = matched.iter().map(|(_, item, _)| item.as_str()).collect();
        let categories = crate::api::shopping_list::list::categorize(conn, &names)?;
        // Taken once the locks are ours, to keep the touch as close to its
        // commit as possible for incremental sync.
        let now = Utc::now();
        for ((id, _, stored), category) in matched.iter().zip(categories) {
            if stored.as_deref() != Some(category.as_str()) {
                diesel::update(items::table.find(id))
                    .set((
                        items::computed_category.eq(&category),
                        items::updated_at.eq(now),
                    ))
                    .execute(conn)?;
            }
        }
        Ok(())
    })
}

/// Record a failed attempt. After a provider failure (an outage, a rate
/// limit, a missing key) every name stays queued for the worker's retry.
/// After an invalid answer a name stays queued until it has had
/// `MAX_ATTEMPTS`, then is marked failed for the status page and its Retry.
/// Returns how many names were marked failed.
async fn save_failed(
    pool: &Arc<DbPool>,
    failed: Vec<String>,
    error: String,
    provider_wide: bool,
) -> Result<usize, String> {
    tracing::warn!(
        names = failed.len(),
        "ingredient name resolution failed: {}",
        error
    );
    if provider_wide {
        return Ok(0);
    }
    let now = Utc::now();
    run_blocking(pool, move |conn| {
        conn.transaction(|conn| {
            let failed = names::table
                .filter(names::name.eq_any(&failed))
                .filter(names::status.eq(PENDING));
            diesel::update(failed.clone())
                .set((
                    names::attempts.eq(names::attempts + 1),
                    names::updated_at.eq(now),
                ))
                .execute(conn)?;
            diesel::update(failed.filter(names::attempts.ge(MAX_ATTEMPTS)))
                .set((names::status.eq(FAILED), names::error.eq(&error)))
                .execute(conn)
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, status: &str, disposition: Option<&str>, key: Option<&str>) -> StoredName {
        StoredName {
            name: name.to_string(),
            status: status.to_string(),
            disposition: disposition.map(str::to_string),
            catalog_key: key.map(str::to_string),
        }
    }

    #[test]
    fn names_the_catalog_knows_are_left_alone() {
        let known = row("all-purpose flour", RESOLVED, Some("not_food"), None);
        assert_eq!(catalog_sync(&known), None);
    }

    #[test]
    fn answers_an_ambiguous_name_cannot_have_are_asked_again() {
        for disposition in ["estimate", "not_food"] {
            let cheese = row("cheese", RESOLVED, Some(disposition), None);
            assert_eq!(catalog_sync(&cheese), Some(CatalogSync::NowAmbiguous));
        }
        let cheese = row("cheese", RESOLVED, Some("entry"), Some("cheddar cheese"));
        assert_eq!(catalog_sync(&cheese), None);
    }

    #[test]
    fn keys_the_catalog_lost_are_asked_again() {
        let stale = row("zorblax paste", RESOLVED, Some("entry"), Some("zorblax"));
        assert_eq!(catalog_sync(&stale), Some(CatalogSync::StaleKey));
        let good = row(
            "zorblax paste",
            RESOLVED,
            Some("entry"),
            Some("all-purpose flour"),
        );
        assert_eq!(catalog_sync(&good), None);
    }

    #[test]
    fn other_answers_and_open_names_are_left_alone() {
        for (status, disposition) in [
            (PENDING, None),
            (FAILED, None),
            (RESOLVED, Some("not_food")),
            (RESOLVED, Some("unknown")),
            (RESOLVED, Some("estimate")),
        ] {
            assert_eq!(
                catalog_sync(&row("zorblax paste", status, disposition, None)),
                None,
                "{status} {disposition:?}"
            );
        }
    }
}
