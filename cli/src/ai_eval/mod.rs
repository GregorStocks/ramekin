//! CLI command: evaluate AI models against committed golden sets.
//!
//! Each suite is one AI use case with a fixed, committed input set in
//! `data/ai-evals/golden/<suite>.json` (written by `--write-golden`, generated
//! deterministically from the committed catalog) and a scorer. A run asks
//! every model through the production prompt and batching, and writes
//! `data/ai-evals/<suite>.md` with one row per model. Responses are cached by
//! model and prompt, so reruns are free and a new model only pays for itself.

mod extraction;
mod judged;

use anyhow::{bail, Context, Result};
use ramekin_core::ai::{
    estimate_ingredient_weights, resolve_ingredient_names, AiConfig, AiError, CachingAiClient,
    NameQuery, NameResolution, Usage,
};
use ramekin_core::catalog::{self, Resolution};
use ramekin_core::nutrition::ESTIMABLE_UNITS;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const GOLDEN_DIR: &str = "data/ai-evals/golden";
const SNAPSHOTS_DIR: &str = "data/pipeline-snapshots";
const RESULTS_DIR: &str = "data/ai-evals";
const USDA: &str = "ramekin-core/src/catalog/data/usda.json";
const CURATED: &str = "ramekin-core/src/catalog/data/curated.json";
/// The production batch size for both ingredient prompts, the default.
pub const BATCH: usize = 40;
/// Candidate keys offered per name, as the server does.
const CANDIDATES: usize = 12;

pub const SUITES: [&str; 10] = [
    "ingredient-weights",
    "food-estimates",
    "ingredient-names",
    "text-extraction",
    "photo-extraction",
    "tags",
    "titles",
    "descriptions",
    "custom-enrich",
    "recipe-photos",
];

/// The suites a person judges (`judged`): their reports need judgments, and
/// their models are general (or, for photos, image) models.
const JUDGED: [&str; 5] = [
    "tags",
    "titles",
    "descriptions",
    "custom-enrich",
    "recipe-photos",
];

// ---------------------------------------------------------------------------
// Golden sets

/// Grams in one `unit` of a USDA food, from its USDA portions.
#[derive(Debug, Serialize, Deserialize)]
struct WeightCase {
    food: String,
    unit: String,
    grams: f64,
}

/// A USDA food presented as a name with no catalog candidates, so the model
/// must estimate it, with USDA's numbers as the answer.
#[derive(Debug, Serialize, Deserialize)]
struct FoodCase {
    name: String,
    kcal_per_100g: f64,
    grams_per_cup: Option<f64>,
    grams_per_piece: Option<f64>,
}

/// A curated name with the candidates the model sees (the name itself left
/// out), and the expected answer: the catalog entry it resolves to, or not
/// food.
#[derive(Debug, Serialize, Deserialize)]
struct NameCase {
    name: String,
    candidates: Vec<String>,
    /// The expected entry's id, or None for not food.
    expected_entry: Option<String>,
}

#[derive(Deserialize)]
struct UsdaFile {
    foods: Vec<UsdaFood>,
}

#[derive(Deserialize)]
struct UsdaFood {
    description: String,
    kcal_per_100g: Option<f64>,
    grams_per_cup: Option<f64>,
    #[serde(default)]
    portions: BTreeMap<String, f64>,
    default_portion: Option<String>,
}

#[derive(Deserialize)]
struct CuratedFile {
    aliases: BTreeMap<String, Option<String>>,
    not_food: BTreeMap<String, String>,
}

fn read_json<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T> {
    let path = path.as_ref();
    let text =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("Failed to parse {}", path.display()))
}

/// A stable pseudo-random order: by hash of the key, so a sample doesn't
/// shift when unrelated rows are added.
fn sample<T>(mut items: Vec<T>, key: impl Fn(&T) -> String, n: usize) -> Vec<T> {
    items.sort_by_cached_key(|item| Sha256::digest(key(item).as_bytes()));
    items.truncate(n);
    items
}

/// Curated not-food section headers whose name alone is a food ("Crust:"
/// above the crust's ingredients). The prompt sees only the name, so no model
/// can tell; in a recipe such a line usually has no amount and isn't counted
/// either way.
const FOOD_NAMED_HEADERS: [&str; 3] = ["crust", "dough", "jam filling"];

/// Plain names only: letters, spaces, and light punctuation, so a case tests
/// recognizing a food rather than surviving parser junk.
fn plain_name(name: &str) -> bool {
    name.len() <= 40
        && name.starts_with(|c: char| c.is_alphabetic())
        && name
            .chars()
            .all(|c| c.is_alphabetic() || matches!(c, ' ' | '-' | ',' | '\''))
}

/// Whether the answered key names the expected food. The catalog has several
/// entries for many foods (a curated "whole-milk ricotta" beside USDA's
/// "cheese, ricotta, whole milk"), so an entry the catalog computes
/// identically counts: the same calories, density and piece weights, so no
/// estimate can differ. Sharing a USDA record isn't enough (dill pickle relish
/// overrides the whole pickles' density), nor are similar calories (a
/// Caramello bar isn't a Butterfinger). The reference is the
/// golden set's stored entry, never the alias's current target, so a catalog
/// change can't quietly change what a committed case means.
fn same_food(key: &str, expected: &str) -> Result<bool> {
    let wanted = match catalog::resolve(expected) {
        Resolution::Entry { entry, .. } if entry.id == expected => entry,
        _ => bail!(
            "Golden entry {expected:?} no longer names a catalog entry; regenerate with make ai-eval-golden"
        ),
    };
    let Resolution::Entry {
        entry: answered, ..
    } = catalog::resolve(key)
    else {
        return Ok(false);
    };
    if answered.id == wanted.id {
        return Ok(true);
    }
    Ok(answered.kcal_per_100g.is_some()
        && answered.kcal_per_100g == wanted.kcal_per_100g
        && answered.grams_per_cup == wanted.grams_per_cup
        && answered.portions == wanted.portions
        && answered.default_portion == wanted.default_portion)
}

fn entry_id(name: &str) -> Option<String> {
    match catalog::resolve(name) {
        Resolution::Entry { entry, .. } => Some(entry.id.clone()),
        _ => None,
    }
}

fn write_golden(root: &Path) -> Result<()> {
    let usda: UsdaFile = read_json(root.join(USDA))?;
    let curated: CuratedFile = read_json(root.join(CURATED))?;
    let eaten =
        |food: &&UsdaFood| food.kcal_per_100g.is_some() && !food.description.starts_with("spices");

    let cups: Vec<WeightCase> = usda
        .foods
        .iter()
        .filter(eaten)
        .filter_map(|food| {
            food.grams_per_cup.map(|grams| WeightCase {
                food: food.description.clone(),
                unit: "cup".into(),
                grams,
            })
        })
        .collect();
    let pieces: Vec<WeightCase> = usda
        .foods
        .iter()
        .filter(eaten)
        .flat_map(|food| {
            food.portions
                .iter()
                .filter(|(unit, _)| *unit != "cup" && ESTIMABLE_UNITS.contains(&unit.as_str()))
                .map(|(unit, grams)| WeightCase {
                    food: food.description.clone(),
                    unit: unit.clone(),
                    grams: *grams,
                })
        })
        .collect();
    let key = |case: &WeightCase| format!("{} | {}", case.food, case.unit);
    let mut weights = sample(cups, key, 60);
    weights.extend(sample(pieces, key, 60));

    let foods: Vec<FoodCase> = usda
        .foods
        .iter()
        .filter(eaten)
        .map(|food| FoodCase {
            name: food.description.clone(),
            kcal_per_100g: food.kcal_per_100g.expect("filtered to foods with calories"),
            grams_per_cup: food.grams_per_cup,
            grams_per_piece: food
                .default_portion
                .as_ref()
                .and_then(|piece| food.portions.get(piece))
                .copied(),
        })
        .collect();
    let foods = sample(foods, |case| case.name.clone(), 80);

    let name_case = |name: &str, expected_entry: Option<String>| -> Option<NameCase> {
        let candidates: Vec<String> = catalog::candidates(name, CANDIDATES + 1)
            .into_iter()
            .filter(|key| *key != name)
            .take(CANDIDATES)
            .map(str::to_string)
            .collect();
        // Answerable only if some candidate names the expected food.
        if let Some(expected) = &expected_entry {
            if !candidates
                .iter()
                .any(|key| entry_id(key).as_ref() == Some(expected))
            {
                return None;
            }
        }
        Some(NameCase {
            name: name.to_string(),
            candidates,
            expected_entry,
        })
    };
    let aliases: Vec<NameCase> = curated
        .aliases
        .iter()
        .filter(|(name, target)| target.is_some() && plain_name(name))
        .filter_map(|(name, _)| name_case(name, Some(entry_id(name)?)))
        .collect();
    let not_food: Vec<NameCase> = curated
        .not_food
        .keys()
        .filter(|name| plain_name(name) && !FOOD_NAMED_HEADERS.contains(&name.as_str()))
        .filter_map(|name| name_case(name, None))
        .collect();
    let mut names = sample(aliases, |case| case.name.clone(), 100);
    names.extend(sample(not_food, |case| case.name.clone(), 30));

    let dir = root.join(GOLDEN_DIR);
    fs::create_dir_all(&dir)?;
    for (suite, json) in [
        (
            "ingredient-weights",
            serde_json::to_string_pretty(&weights)?,
        ),
        ("food-estimates", serde_json::to_string_pretty(&foods)?),
        ("ingredient-names", serde_json::to_string_pretty(&names)?),
        (
            "text-extraction",
            serde_json::to_string_pretty(&extraction::write_golden(root)?)?,
        ),
    ]
    .into_iter()
    .chain(judged::write_golden(root)?)
    {
        let path = dir.join(format!("{suite}.json"));
        fs::write(&path, json + "\n")?;
        tracing::info!("Wrote {}", path.display());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Models, pricing, and spend

/// Dollars per token, prompt and completion, from OpenRouter's model list.
struct Price {
    prompt: f64,
    completion: f64,
}

async fn prices(models: &[String]) -> Result<HashMap<String, Price>> {
    #[derive(Deserialize)]
    struct List {
        data: Vec<Model>,
    }
    #[derive(Deserialize)]
    struct Model {
        id: String,
        pricing: Pricing,
    }
    #[derive(Deserialize)]
    struct Pricing {
        prompt: String,
        completion: String,
    }
    // The default list leaves out models that only make images.
    let list: List = reqwest::get("https://openrouter.ai/api/v1/models?output_modalities=all")
        .await?
        .error_for_status()?
        .json()
        .await
        .context("Failed to read OpenRouter's model list")?;
    let mut prices = HashMap::new();
    for model in models {
        let (id, _) = judged::split_quality(model);
        let Some(found) = list.data.iter().find(|m| m.id == id) else {
            bail!("OpenRouter has no model {model:?}");
        };
        prices.insert(
            model.clone(),
            Price {
                prompt: found.pricing.prompt.parse()?,
                completion: found.pricing.completion.parse()?,
            },
        );
    }
    Ok(prices)
}

#[derive(Default)]
struct Spend {
    calls: usize,
    /// Batches cut off by the production max_tokens, which would fail in
    /// production too (where they count as provider errors).
    truncated: usize,
    /// Why items were rejected, first few, for the report.
    rejections: Vec<String>,
    /// Calls whose answer was rejected (invalid or truncated). The provider
    /// billed them, but an error carries no usage, so they're counted here
    /// instead of in the cost.
    rejected_calls: usize,
    prompt_tokens: u64,
    completion_tokens: u64,
    /// Dollars the provider reported for calls that carry no token usage
    /// (image generation, whose image tokens are priced apart from text).
    reported_dollars: f64,
}

impl Spend {
    fn add(&mut self, usage: &Usage) {
        self.calls += 1;
        self.prompt_tokens += u64::from(usage.prompt_tokens);
        self.completion_tokens += u64::from(usage.completion_tokens);
    }

    fn dollars(&self, price: &Price) -> f64 {
        self.prompt_tokens as f64 * price.prompt
            + self.completion_tokens as f64 * price.completion
            + self.reported_dollars
    }
}

/// A client with the use case's production settings (`for_ingredients`,
/// `for_extraction`: its timeout), with the candidate model in place of the
/// use case's model.
fn client_for(model: &str, use_case: fn(&AiConfig) -> AiConfig) -> Result<CachingAiClient> {
    let mut config =
        use_case(&AiConfig::from_env().context("AI is not configured (OPENROUTER_API_KEY)")?);
    config.model = model.to_string();
    Ok(CachingAiClient::new(config))
}

/// How a production worker retries a batch whose answer was rejected.
#[derive(Clone, Copy)]
enum Split {
    /// Each half on its own, down to single items (the weights worker).
    Halves,
    /// Each item on its own (the names worker).
    Singles,
}

/// How many times a batch is asked before a provider error (a request
/// timeout, an upstream refusal, a rate limit) counts as its answer.
const PROVIDER_ATTEMPTS: u32 = 3;

/// The wait before the first retry of a provider error, doubled each retry.
const PROVIDER_RETRY_DELAY: Duration = if cfg!(test) {
    Duration::ZERO
} else {
    Duration::from_secs(5)
};

/// Batches whose provider errors outlast their retries before the run stops:
/// past this the provider (or the key) is down, and counting every remaining
/// case invalid would only write a misleading report. A total, not a run in
/// a row, since cached answers succeed through an outage.
const MAX_PROVIDER_FAILURES: usize = 5;

/// Ask `items` `batch_size` at a time, retrying a rejected batch (invalid, or
/// truncated) the way the production worker does; a rejected single item is
/// invalid, with no second chance, as in production. A provider error is
/// retried a few times, then fails the whole batch (production doesn't split
/// on those either), so one flaky case doesn't abort the suite.
/// Configuration errors, and provider errors on `MAX_PROVIDER_FAILURES`
/// batches or on more than half of them, stop the run.
async fn in_batches<I: Clone, A>(
    items: &[I],
    batch_size: usize,
    split: Split,
    spend: &mut Spend,
    mut ask: impl AsyncFnMut(&[I]) -> Result<(A, Usage), AiError>,
    mut take: impl FnMut(A),
) -> Result<usize> {
    let mut invalid = 0;
    let mut provider_failures = 0;
    let mut last_provider_error = None;
    // Batches asked, counting those split off a rejected one.
    let mut batches = 0;
    let mut queue: VecDeque<Vec<I>> = items.chunks(batch_size).map(<[I]>::to_vec).collect();
    while let Some(batch) = queue.pop_front() {
        batches += 1;
        let e = match ask_with_retries(&mut ask, &batch).await {
            Ok((answers, usage)) => {
                spend.add(&usage);
                take(answers);
                continue;
            }
            Err(e) => e,
        };
        let provider_failure = match e {
            // Only photo generation times out this way: a slow image model
            // fails that photo in production too, so it's the model's miss.
            _ if e.is_answer_specific() || matches!(e, AiError::Timeout(_)) => false,
            AiError::Api(_) | AiError::RateLimited(_) => true,
            _ => return Err(e.into()),
        };
        if provider_failure {
            provider_failures += 1;
            if provider_failures >= MAX_PROVIDER_FAILURES {
                return Err(anyhow::Error::from(e).context(format!(
                    "{provider_failures} batches failed with provider errors"
                )));
            }
        }
        spend.rejected_calls += 1;
        spend.truncated += usize::from(matches!(e, AiError::Truncated(_)));
        if provider_failure || batch.len() == 1 {
            invalid += batch.len();
            if spend.rejections.len() < 5 {
                spend
                    .rejections
                    .push(e.to_string().chars().take(300).collect());
            }
            if provider_failure {
                last_provider_error = Some(e);
            }
            continue;
        }
        match split {
            Split::Halves => {
                let mut first = batch;
                let second = first.split_off(first.len() / 2);
                queue.push_front(second);
                queue.push_front(first);
            }
            Split::Singles => {
                for item in batch.into_iter().rev() {
                    queue.push_front(vec![item]);
                }
            }
        }
    }
    // A suite of fewer batches than the limit can fail most of them.
    match last_provider_error {
        Some(e) if provider_failures * 2 > batches => Err(anyhow::Error::from(e).context(format!(
            "{provider_failures} of {batches} batches failed with provider errors"
        ))),
        _ => Ok(invalid),
    }
}

/// `ask(batch)`, asked again after a provider error up to
/// `PROVIDER_ATTEMPTS` times in all.
async fn ask_with_retries<I, A>(
    ask: &mut impl AsyncFnMut(&[I]) -> Result<(A, Usage), AiError>,
    batch: &[I],
) -> Result<(A, Usage), AiError> {
    let mut delay = PROVIDER_RETRY_DELAY;
    let mut attempt = 1;
    loop {
        match ask(batch).await {
            Err(e @ (AiError::Api(_) | AiError::RateLimited(_))) if attempt < PROVIDER_ATTEMPTS => {
                tracing::warn!("Provider error (attempt {attempt}), retrying in {delay:?}: {e}");
                tokio::time::sleep(delay).await;
                delay *= 2;
                attempt += 1;
            }
            result => return result,
        }
    }
}

// ---------------------------------------------------------------------------
// Scoring helpers

/// Calories per 100 g below which a calorie error is measured against this
/// instead of the truth.
const KCAL_ERROR_FLOOR: f64 = 20.0;

/// Absolute relative error of an estimate, as a fraction of the truth.
fn error(estimate: f64, truth: f64) -> f64 {
    (estimate - truth).abs() / truth
}

fn percentile(errors: &[f64], p: f64) -> Option<f64> {
    if errors.is_empty() {
        return None;
    }
    let mut sorted = errors.to_vec();
    sorted.sort_by(f64::total_cmp);
    let index = ((sorted.len() - 1) as f64 * p).round() as usize;
    Some(sorted[index])
}

fn pct(numerator: usize, denominator: usize) -> String {
    if denominator == 0 {
        return "–".into();
    }
    format!("{:.0}%", 100.0 * numerator as f64 / denominator as f64)
}

fn err_pct(value: Option<f64>) -> String {
    value.map_or("–".into(), |v| format!("{:.0}%", 100.0 * v))
}

fn within(errors: &[f64], limit: f64, of: usize) -> String {
    pct(errors.iter().filter(|e| **e <= limit).count(), of)
}

/// Every case's answer for one model, for reading beyond the report:
/// `logs/ai-evals/<suite>/<model>.json` (not committed).
fn dump(suite: &str, model: &str, rows: Vec<serde_json::Value>) -> Result<()> {
    let dir = Path::new("logs/ai-evals").join(suite);
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", model.replace('/', "_")));
    fs::write(&path, serde_json::to_string_pretty(&rows)? + "\n")
        .with_context(|| format!("Failed to write {}", path.display()))
}

/// One model's row and its worst misses, for the report.
struct ModelResult {
    model: String,
    cells: Vec<String>,
    misses: Vec<(f64, String)>,
}

fn worst(mut misses: Vec<(f64, String)>) -> Vec<(f64, String)> {
    misses.sort_by(|a, b| b.0.total_cmp(&a.0));
    misses.truncate(5);
    misses
}

// ---------------------------------------------------------------------------
// Suites

async fn eval_weights(
    model: &str,
    cases: &[WeightCase],
    batch_size: usize,
    spend: &mut Spend,
) -> Result<ModelResult> {
    let client = client_for(model, AiConfig::for_ingredients)?;
    let items: Vec<(String, String)> = cases
        .iter()
        .map(|case| (case.food.clone(), case.unit.clone()))
        .collect();
    let mut answers = HashMap::new();
    let invalid = in_batches(
        &items,
        batch_size,
        Split::Halves,
        spend,
        async |batch: &[(String, String)]| {
            estimate_ingredient_weights(&client, batch, false)
                .await
                .map(|result| (result.weights, result.usage))
        },
        |weights| answers.extend(weights),
    )
    .await?;
    dump(
        "ingredient-weights",
        model,
        cases
            .iter()
            .map(|case| {
                serde_json::json!({
                    "food": case.food,
                    "unit": case.unit,
                    "usda_grams": case.grams,
                    "answer": answers.get(&(case.food.clone(), case.unit.clone())),
                })
            })
            .collect(),
    )?;
    let mut errors = Vec::new();
    let mut nulls = 0;
    let mut misses = Vec::new();
    for case in cases {
        match answers.get(&(case.food.clone(), case.unit.clone())) {
            Some(Some(grams)) => {
                let e = error(*grams, case.grams);
                errors.push(e);
                misses.push((
                    e,
                    format!(
                        "{} | {}: {grams} g (USDA {} g)",
                        case.food, case.unit, case.grams
                    ),
                ));
            }
            Some(None) => nulls += 1,
            None => {}
        }
    }
    let n = cases.len();
    Ok(ModelResult {
        model: model.into(),
        cells: vec![
            within(&errors, 0.2, n),
            within(&errors, 0.5, n),
            err_pct(percentile(&errors, 0.5)),
            err_pct(percentile(&errors, 0.9)),
            pct(nulls, n),
            pct(invalid, n),
        ],
        misses: worst(misses),
    })
}

async fn eval_foods(
    model: &str,
    cases: &[FoodCase],
    batch_size: usize,
    spend: &mut Spend,
) -> Result<ModelResult> {
    let client = client_for(model, AiConfig::for_ingredients)?;
    let queries: Vec<String> = cases.iter().map(|case| case.name.clone()).collect();
    let mut answers = HashMap::new();
    let invalid = in_batches(
        &queries,
        batch_size,
        Split::Singles,
        spend,
        async |batch: &[String]| {
            let batch: Vec<NameQuery> = batch
                .iter()
                .map(|name| NameQuery {
                    name: name.clone(),
                    candidates: Vec::new(),
                    ambiguous: false,
                })
                .collect();
            resolve_ingredient_names(&client, &batch, false)
                .await
                .map(|result| (result.resolutions, result.usage))
        },
        |resolutions| answers.extend(resolutions),
    )
    .await?;
    dump(
        "food-estimates",
        model,
        cases
            .iter()
            .map(|case| {
                serde_json::json!({
                    "name": case.name,
                    "usda": [case.kcal_per_100g, case.grams_per_cup, case.grams_per_piece],
                    "answer": answers.get(&case.name).map(|answer| format!("{answer:?}")),
                })
            })
            .collect(),
    )?;
    let (mut kcal, mut cups, mut pieces) = (Vec::new(), Vec::new(), Vec::new());
    let (mut cup_cases, mut piece_cases, mut piece_answers, mut unknown) = (0, 0, 0, 0);
    let mut misses = Vec::new();
    for case in cases {
        cup_cases += usize::from(case.grams_per_cup.is_some());
        piece_cases += usize::from(case.grams_per_piece.is_some());
        let Some(answer) = answers.get(&case.name) else {
            continue;
        };
        let NameResolution::Estimate(estimate) = answer else {
            unknown += 1;
            continue;
        };
        // Against at least 20 kcal/100 g, so a diet drink guessed at 0
        // instead of 1 isn't a 100% miss.
        let e = (estimate.kcal_per_100g - case.kcal_per_100g).abs()
            / case.kcal_per_100g.max(KCAL_ERROR_FLOOR);
        kcal.push(e);
        misses.push((
            e,
            format!(
                "{}: {} kcal/100 g (USDA {})",
                case.name, estimate.kcal_per_100g, case.kcal_per_100g
            ),
        ));
        if let (Some(truth), Some(guess)) = (case.grams_per_cup, estimate.grams_per_cup) {
            cups.push(error(guess, truth));
        }
        if let (Some(truth), Some(guess)) = (case.grams_per_piece, estimate.grams_per_piece) {
            pieces.push(error(guess, truth));
            piece_answers += 1;
        }
    }
    let n = cases.len();
    Ok(ModelResult {
        model: model.into(),
        cells: vec![
            within(&kcal, 0.2, n),
            err_pct(percentile(&kcal, 0.5)),
            err_pct(percentile(&kcal, 0.9)),
            within(&cups, 0.25, cup_cases),
            pct(piece_answers, piece_cases),
            within(&pieces, 0.25, piece_answers),
            pct(unknown, n),
            pct(invalid, n),
        ],
        misses: worst(misses),
    })
}

async fn eval_names(
    model: &str,
    cases: &[NameCase],
    batch_size: usize,
    spend: &mut Spend,
) -> Result<ModelResult> {
    let client = client_for(model, AiConfig::for_ingredients)?;
    let indexes: Vec<usize> = (0..cases.len()).collect();
    let mut answers = HashMap::new();
    let invalid = in_batches(
        &indexes,
        batch_size,
        Split::Singles,
        spend,
        async |batch: &[usize]| {
            let batch: Vec<NameQuery> = batch
                .iter()
                .map(|i| NameQuery {
                    name: cases[*i].name.clone(),
                    candidates: cases[*i].candidates.clone(),
                    ambiguous: false,
                })
                .collect();
            resolve_ingredient_names(&client, &batch, false)
                .await
                .map(|result| (result.resolutions, result.usage))
        },
        |resolutions| answers.extend(resolutions),
    )
    .await?;
    dump(
        "ingredient-names",
        model,
        cases
            .iter()
            .map(|case| {
                serde_json::json!({
                    "name": case.name,
                    "expected": case.expected_entry,
                    "candidates": case.candidates,
                    "answer": answers.get(&case.name).map(|answer| format!("{answer:?}")),
                })
            })
            .collect(),
    )?;
    let (mut foods, mut not_foods) = (0, 0);
    let (mut right_food, mut wrong_food, mut unknown, mut right_not_food) = (0, 0, 0, 0);
    let mut misses = Vec::new();
    for case in cases {
        let answer = answers.get(&case.name);
        match &case.expected_entry {
            Some(expected) => {
                foods += 1;
                match answer {
                    Some(NameResolution::Entry(key)) if same_food(key, expected)? => {
                        right_food += 1
                    }
                    Some(NameResolution::Unknown) => unknown += 1,
                    Some(other) => {
                        wrong_food += 1;
                        misses.push((
                            1.0,
                            format!("{}: {other:?} (expected {expected})", case.name),
                        ));
                    }
                    None => {}
                }
            }
            None => {
                not_foods += 1;
                match answer {
                    Some(NameResolution::NotFood) => right_not_food += 1,
                    Some(other) => {
                        misses.push((1.0, format!("{}: {other:?} (expected not food)", case.name)))
                    }
                    None => {}
                }
            }
        }
    }
    Ok(ModelResult {
        model: model.into(),
        cells: vec![
            pct(right_food, foods),
            pct(wrong_food, foods),
            pct(unknown, foods),
            pct(right_not_food, not_foods),
            pct(invalid, cases.len()),
        ],
        misses: worst(misses),
    })
}

// ---------------------------------------------------------------------------
// Report

struct SuiteSpec {
    title: &'static str,
    about: &'static str,
    columns: &'static [&'static str],
    /// Whether production asks many items per call, so `BATCH=n` applies.
    batched: bool,
}

/// How both extraction suites score, for their reports (a macro so `concat!`
/// can use it).
macro_rules! extraction_scoring {
    () => {
        "Ingredient lines are compared whole after lowercasing, writing fractions as 1/2, and dropping bullets and extra spaces; section headings are colon-terminated lines and count too. Found is the expected lines answered, extra the answered lines not expected (prose, a neighbouring recipe). Instructions are compared as words: recall is the expected words kept in the instructions (or notes, for a moved tip, once the instructions hold at least half), precision the answered instruction words that are expected or allowed (a variation or headnote the source contains). Other text kept is the share of the page's other text (a description, headnote or variation) found in the answer's description, notes or instructions. Invented fields counts difficulty, source, categories (the draft's tags) and rating with words the source never uses, rating unless the source rates it, and servings, times and nutrition with such words or a number the source doesn't state as that kind of quantity (a total summed from the steps, or the 4 of \"serves 4\" given as minutes), judged by the word each number phrase measures (\"5 to 6 minutes\", \"serves 4\"; not \"130 degrees\"). A time counts as stated only where the source labels one (\"Prep time:\", \"Total time\", \"Ready in\"), so a step's \"chill 30 minutes\" given as the total time is invented. Each stated quantity backs one answered value. Counted over all cases. Servings, times and nutrition kept is the share of the servings, times and nutrition the source states that the answer gives with the same numbers (a time in any time field; nutrition in the nutrition field, not the notes). Unsourced note words is the share of the description and notes words that appear nowhere in the source. Columns are over the recipes a model answered validly."
    };
}

/// How the judged suites are judged, for their reports.
macro_rules! judging {
    () => {
        "Judged by a person on a blind page (`logs/ai-evals/<suite>/judge.html`, written by each run): model names hidden, answers shuffled, identical answers merged. Unjudged counts the valid answers no judgment covers yet; judge them and rerun."
    };
}

/// How the verdict suites score.
macro_rules! verdicts {
    () => {
        "Each distinct answer is judged best, good or bad (`data/ai-evals/judgments/<suite>.json`, keyed by a hash of the answer, so a model giving an answer already judged reuses its judgment). Good counts best and good answers, over the model's judged answers; best is the share judged among the best for their case."
    };
}

fn spec(suite: &str) -> SuiteSpec {
    match suite {
        "ingredient-weights" => SuiteSpec {
            title: "Ingredient weights",
            about: "Grams in one unit of a catalog food (`estimate_ingredient_weights`), against USDA: 60 densities (cup) and 60 piece weights in estimable units. Error is |estimate − USDA| / USDA; null means the model said there's no typical weight.",
            columns: &["Within 20%", "Within 50%", "Median error", "P90 error", "Null", "Invalid"],
            batched: true,
        },
        "food-estimates" => SuiteSpec {
            title: "Food estimates",
            about: "Calories (and cup and piece weights) for a food with no catalog candidates (`resolve_ingredient_names`, answer \"estimate\"), against USDA for 80 foods given by their USDA description. Calorie error is against at least 20 kcal/100 g, so near-zero foods don't dominate. Cup and piece columns count foods where USDA has the weight. The piece is USDA's default portion, which is sometimes a whole pizza, roast or bird where a model gives one slice or serving, so with ~25 cases that column is low-signal.",
            columns: &["kcal within 20%", "kcal median error", "kcal P90 error", "Cup within 25%", "Piece answered", "Piece within 25% of answered", "Unknown", "Invalid"],
            batched: true,
        },
        "ingredient-names" => SuiteSpec {
            title: "Ingredient names",
            about: "Picking the catalog food a name means from its candidates (`resolve_ingredient_names`), for 100 curated aliases (correct if the answer resolves to the alias's food or another entry the catalog computes identically, with the same calories, density and piece weights, since the catalog has several entries for many foods; the alias itself is not offered) and 30 curated not-food names. Some curated aliases are judgment calls (delicata squash counts as acorn squash), so a sensible answer can score as wrong: compare models with each other rather than reading this as absolute accuracy.",
            columns: &["Correct food", "Wrong food", "Unknown", "Not food right", "Invalid"],
            batched: true,
        },
        "text-extraction" => SuiteSpec {
            title: "Text extraction",
            about: concat!(
                "A recipe read from pasted text (`extract_recipe_from_text`), for 30 pipeline snapshots rendered as the plain text a user would paste (a third wrapped in blog chatter, a third with no description or servings) and 3 texts that aren't recipes. ",
                extraction_scoring!(),
                " \"Recipes with warnings\" counts valid recipes answered with any warning: text import shows them to the user, so a warning on a sound recipe is noise. \"Not a recipe: left empty\" counts the non-recipes answered with a warning and every field a user would see (title, ingredients, instructions, description, servings, times, nutrition, notes, difficulty, source, categories, rating) empty."
            ),
            columns: extraction::TEXT_COLUMNS,
            batched: false,
        },
        "photo-extraction" => SuiteSpec {
            title: "Photo extraction",
            about: concat!(
                "A recipe read from photos of real pages (`extract_recipe_from_photos`), from `data/ai-evals/photos/` with hand-checked transcriptions; each case's `note` in the golden set says what makes it hard. The photos are sent as uploaded, EXIF rotation and all, as photo import does. With 5 cases, one case moves a column by 20 points. ",
                extraction_scoring!(),
                " Kalbi Burgers has no ingredient list (its ingredients are bold words in the steps), so its expected lines are those words: a judgment call."
            ),
            columns: extraction::PHOTO_COLUMNS,
            batched: false,
        },
        "tags" => SuiteSpec {
            title: "Tags",
            about: concat!(
                "Tags picked from a user's tags for a recipe (`suggest_tags`), for 20 pipeline snapshots and a fixed vocabulary of 33 tags (`TAG_VOCABULARY`), against the tags a person ticked as applying (`data/ai-evals/judgments/tags.json`). Exact is the recipes answered with exactly the judged tags; precision and recall count tags over all judged recipes. ",
                judging!()
            ),
            columns: judged::TAG_COLUMNS,
            batched: false,
        },
        "titles" => SuiteSpec {
            title: "Titles",
            about: concat!(
                "A tidied recipe title (`normalize_title`) for 20 pipeline snapshots: 14 with decoration to remove (praise, \"recipe\", a parenthetical, a subtitle) and 6 already plain, which should come back unchanged. ",
                judging!(),
                " ",
                verdicts!()
            ),
            columns: judged::VERDICT_COLUMNS,
            batched: false,
        },
        "descriptions" => SuiteSpec {
            title: "Descriptions",
            about: concat!(
                "A menu-style description (`generate_description`) for 15 pipeline snapshots. ",
                judging!(),
                " ",
                verdicts!()
            ),
            columns: judged::VERDICT_COLUMNS,
            batched: false,
        },
        "custom-enrich" => SuiteSpec {
            title: "Custom enrich",
            about: concat!(
                "A recipe changed by a user's instruction (`custom_enrich`), for 10 pipeline snapshots each paired with an instruction that suits it (`ENRICH_CASES`: make it vegetarian, halve it, convert to metric…). An answer that doesn't parse as the server's recipe shape is invalid, as in production. ",
                judging!(),
                " ",
                verdicts!()
            ),
            columns: judged::VERDICT_COLUMNS,
            batched: false,
        },
        "recipe-photos" => SuiteSpec {
            title: "Recipe photos",
            about: concat!(
                "A generated recipe photo (`generate_recipe_photo`; the models are image models, `model@high` asking for that quality) for 10 hand-picked pipeline snapshots (`PHOTO_CASES`): dishes whose real look is crisp, structured or colorful, so a model that renders food as brown mush is caught. Photos are cached under the AI cache directory by model and prompt, since the provider doesn't cache them. Cost is what OpenRouter reported for each photo. ",
                judging!(),
                " ",
                verdicts!()
            ),
            columns: judged::VERDICT_COLUMNS,
            batched: false,
        },
        other => unreachable!("unknown suite {other}"),
    }
}

fn render(suite: &str, cases: usize, batch_size: usize, rows: &[Row]) -> String {
    let spec = spec(suite);
    let mut out = format!(
        "# AI eval: {}\n\nWritten by `make ai-eval`. {}\n\nGolden set: `{GOLDEN_DIR}/{suite}.json` ({cases} cases), {}. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.\n\n",
        spec.title,
        spec.about,
        if spec.batched {
            format!("asked {batch_size} per call (production asks {BATCH}). Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), or a provider error that outlasted its retries; a rejected batch is retried as the production worker does (in halves for weights, item by item for names), and a rejected single item is invalid, as is every item of a batch the provider kept failing (production doesn't split those)")
        } else {
            "one call per case, as in production. Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), or a provider error that outlasted its retries, and are invalid: production doesn't retry them".to_string()
        }
    );
    let _ = writeln!(
        out,
        "| Model | {} | Rejected calls | Truncated calls | Cost of accepted calls |",
        spec.columns.join(" | ")
    );
    let _ = writeln!(
        out,
        "| --- |{} ---: | ---: | ---: |",
        " ---: |".repeat(spec.columns.len())
    );
    for (result, dollars, rejected, truncated, _) in rows {
        let _ = writeln!(
            out,
            "| {} | {} | {rejected} | {truncated} | ${dollars:.4} |",
            result.model,
            result.cells.join(" | ")
        );
    }
    out.push_str(
        "\n## Rejected answers\n\nThe first few items each model answered invalidly even alone.\n",
    );
    for (result, _, _, _, rejections) in rows {
        if rejections.is_empty() {
            continue;
        }
        let _ = writeln!(out, "\n### {}\n", result.model);
        for rejection in rejections {
            let _ = writeln!(out, "- {}", rejection.replace('\n', " "));
        }
    }
    out.push_str("\n## Worst misses\n");
    for (result, _, _, _, _) in rows {
        let _ = writeln!(out, "\n### {}\n", result.model);
        if result.misses.is_empty() {
            out.push_str("None.\n");
        }
        for (_, miss) in &result.misses {
            let _ = writeln!(out, "- {miss}");
        }
    }
    out
}

/// A model's report row: its result, cost, rejected and truncated calls, and
/// first rejections.
type Row = (ModelResult, f64, usize, usize, Vec<String>);

fn row(suite: &str, result: ModelResult, spend: Spend, price: &Price) -> Row {
    let dollars = spend.dollars(price);
    tracing::info!(
        suite,
        model = result.model,
        calls = spend.calls,
        dollars,
        "evaluated"
    );
    (
        result,
        dollars,
        spend.rejected_calls,
        spend.truncated,
        spend.rejections,
    )
}

pub async fn run(
    root: &Path,
    suite: &str,
    models: &[String],
    batch_size: usize,
    golden: bool,
) -> Result<()> {
    if golden {
        return write_golden(root);
    }
    if models.is_empty() {
        bail!("Pass at least one model (MODELS=a,b)");
    }
    if batch_size == 0 {
        bail!("The batch size must be at least 1");
    }
    let suites: Vec<&str> = match suite {
        // Recipe photos take image models, so "all" (text models) leaves
        // them out.
        "all" => SUITES
            .into_iter()
            .filter(|s| *s != "recipe-photos")
            .collect(),
        name if SUITES.contains(&name) => vec![name],
        other => bail!("Unknown suite {other:?}; one of {SUITES:?} or all"),
    };
    // Configuration is checked once, up front: it's the run's problem, not a
    // model's.
    AiConfig::from_env().context("AI is not configured (OPENROUTER_API_KEY)")?;
    let prices = prices(models).await?;
    for suite in suites {
        let path: PathBuf = root.join(GOLDEN_DIR).join(format!("{suite}.json"));
        let mut rows = Vec::new();
        let mut cases = 0;
        // Read once for every model: they're megabytes.
        let photos = match suite {
            "photo-extraction" => {
                extraction::load_photos(root, &read_json::<Vec<extraction::PhotoCase>>(&path)?)?
            }
            _ => Vec::new(),
        };
        if JUDGED.contains(&suite) {
            // A judged suite asks every model before it can write the page
            // of their answers, so it runs as a whole.
            let run = judged::eval(root, suite, models)
                .await
                .with_context(|| format!("{suite} failed"))?;
            cases = run.cases;
            for (result, spend) in run.results {
                let price = &prices[&result.model];
                rows.push(row(suite, result, spend, price));
            }
        } else {
            for model in models {
                let mut spend = Spend::default();
                let outcome = match suite {
                    "ingredient-weights" => {
                        let golden: Vec<WeightCase> = read_json(&path)?;
                        cases = golden.len();
                        eval_weights(model, &golden, batch_size, &mut spend).await
                    }
                    "food-estimates" => {
                        let golden: Vec<FoodCase> = read_json(&path)?;
                        cases = golden.len();
                        eval_foods(model, &golden, batch_size, &mut spend).await
                    }
                    "ingredient-names" => {
                        let golden: Vec<NameCase> = read_json(&path)?;
                        cases = golden.len();
                        eval_names(model, &golden, batch_size, &mut spend).await
                    }
                    "text-extraction" => {
                        let golden: Vec<extraction::TextCase> = read_json(&path)?;
                        cases = golden.len();
                        extraction::eval_text(model, &golden, &mut spend).await
                    }
                    "photo-extraction" => {
                        let golden: Vec<extraction::PhotoCase> = read_json(&path)?;
                        cases = golden.len();
                        extraction::eval_photos(model, &golden, &photos, &mut spend).await
                    }
                    other => unreachable!("unknown suite {other}"),
                };
                // A failing model (a timeout) stops the run before any report is
                // overwritten; everything already answered is cached, so a rerun
                // without it (or after fixing the cause) is nearly free.
                let result = outcome.with_context(|| format!("{suite} / {model} failed"))?;
                rows.push(row(suite, result, spend, &prices[model]));
            }
        }
        // Production's batch size is the main report; others sit beside it.
        let name = if batch_size == BATCH || !spec(suite).batched {
            format!("{suite}.md")
        } else {
            format!("{suite}-batch-{batch_size}.md")
        };
        let out = root.join(RESULTS_DIR).join(name);
        fs::write(&out, render(suite, cases, batch_size, &rows))
            .with_context(|| format!("Failed to write {}", out.display()))?;
        tracing::info!("Wrote {}", out.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramekin_core::ai::ConfigError;

    /// Runs `in_batches` over `items` with an `ask` that answers each batch
    /// with its items, or the error `fail` gives for it. Returns the result,
    /// the items answered, the spend, and how many calls were made.
    async fn run(
        items: &[u32],
        batch_size: usize,
        split: Split,
        mut fail: impl FnMut(&[u32], usize) -> Option<AiError>,
    ) -> (Result<usize>, Vec<u32>, Spend, usize) {
        let mut spend = Spend::default();
        let mut answered = Vec::new();
        let mut calls = 0;
        let result = in_batches(
            items,
            batch_size,
            split,
            &mut spend,
            async |batch: &[u32]| {
                calls += 1;
                match fail(batch, calls) {
                    Some(e) => Err(e),
                    None => Ok((batch.to_vec(), Usage::default())),
                }
            },
            |answers| answered.extend(answers),
        )
        .await;
        (result, answered, spend, calls)
    }

    fn timed_out() -> AiError {
        AiError::Api("Request timed out after 60s".into())
    }

    #[tokio::test]
    async fn a_transient_provider_error_is_retried() {
        let (result, answered, spend, calls) = run(&[1, 2], 2, Split::Halves, |_, call| {
            (call == 1).then(timed_out)
        })
        .await;

        assert_eq!(result.unwrap(), 0);
        assert_eq!(answered, [1, 2]);
        assert_eq!(calls, 2);
        assert_eq!(spend.rejected_calls, 0);
    }

    #[tokio::test]
    async fn a_persistent_provider_error_is_an_invalid_answer() {
        let blocked = "Provider error (code 403): Output blocked by content filtering policy";
        let (result, answered, spend, calls) = run(&[1, 2], 1, Split::Singles, |batch, _| {
            (batch == [1]).then(|| AiError::Api(blocked.into()))
        })
        .await;

        assert_eq!(result.unwrap(), 1);
        assert_eq!(answered, [2]);
        assert_eq!(calls, PROVIDER_ATTEMPTS as usize + 1);
        assert_eq!(spend.rejected_calls, 1);
        assert_eq!(spend.rejections, [format!("API error: {blocked}")]);
    }

    #[tokio::test]
    async fn a_persistent_provider_error_fails_the_whole_batch_unsplit() {
        // As in production, which splits only on answer-specific errors.
        let (result, answered, spend, calls) = run(&[1, 2, 3, 4], 2, Split::Halves, |batch, _| {
            batch.contains(&3).then(timed_out)
        })
        .await;

        assert_eq!(result.unwrap(), 2);
        assert_eq!(answered, [1, 2]);
        assert_eq!(calls, 1 + PROVIDER_ATTEMPTS as usize);
        assert_eq!(spend.rejected_calls, 1);
    }

    #[tokio::test]
    async fn a_configuration_error_stops_the_run() {
        let (result, _, _, calls) = run(&[1, 2], 1, Split::Singles, |_, _| {
            Some(AiError::Config(ConfigError::PlaceholderApiKey))
        })
        .await;

        assert!(result.is_err());
        assert_eq!(calls, 1);
    }

    #[tokio::test]
    async fn a_provider_outage_on_a_short_suite_stops_the_run() {
        let (result, _, _, calls) = run(&[1, 2], 1, Split::Singles, |_, _| Some(timed_out())).await;

        let message = format!("{:#}", result.unwrap_err());
        assert!(message.contains("2 of 2 batches"), "{message}");
        assert_eq!(calls, 2 * PROVIDER_ATTEMPTS as usize);
    }

    #[tokio::test]
    async fn split_batches_count_toward_the_provider_failure_share() {
        // The one batch is rejected and split into four singles, one of
        // which fails at the provider: one failure in five batches asked.
        let (result, answered, _, _) =
            run(&[1, 2, 3, 4], 4, Split::Singles, |batch, _| match batch {
                [_, _, ..] => Some(AiError::ParseError("bad json".into())),
                [3] => Some(timed_out()),
                _ => None,
            })
            .await;

        assert_eq!(result.unwrap(), 1);
        assert_eq!(answered, [1, 2, 4]);
    }

    #[tokio::test]
    async fn answers_between_provider_failures_dont_hide_an_outage() {
        // As cached answers keep coming during an outage.
        let items: Vec<u32> = (0..20).collect();
        let (result, answered, _, _) = run(&items, 1, Split::Singles, |batch, _| {
            (batch[0] % 2 == 0).then(timed_out)
        })
        .await;

        let message = format!("{:#}", result.unwrap_err());
        assert!(message.contains("5 batches failed"), "{message}");
        assert_eq!(answered, [1, 3, 5, 7]);
    }

    #[tokio::test]
    async fn a_provider_outage_stops_the_run() {
        let items: Vec<u32> = (0..20).collect();
        let (result, _, _, calls) = run(&items, 1, Split::Singles, |_, _| Some(timed_out())).await;

        let message = format!("{:#}", result.unwrap_err());
        assert!(message.contains("5 batches failed"), "{message}");
        assert_eq!(calls, MAX_PROVIDER_FAILURES * PROVIDER_ATTEMPTS as usize);
    }
}
