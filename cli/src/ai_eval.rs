//! CLI command: evaluate AI models against committed golden sets.
//!
//! Each suite is one AI use case with a fixed, committed input set in
//! `data/ai-evals/golden/<suite>.json` (written by `--write-golden`, generated
//! deterministically from the committed catalog) and a scorer. A run asks
//! every model through the production prompt and batching, and writes
//! `data/ai-evals/<suite>.md` with one row per model. Responses are cached by
//! model and prompt, so reruns are free and a new model only pays for itself.

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

const GOLDEN_DIR: &str = "data/ai-evals/golden";
const RESULTS_DIR: &str = "data/ai-evals";
const USDA: &str = "ramekin-core/src/catalog/data/usda.json";
const CURATED: &str = "ramekin-core/src/catalog/data/curated.json";
/// The production batch size for both ingredient prompts, the default.
pub const BATCH: usize = 40;
/// Candidate keys offered per name, as the server does.
const CANDIDATES: usize = 12;

pub const SUITES: [&str; 3] = ["ingredient-weights", "food-estimates", "ingredient-names"];

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

/// Plain names only: letters, spaces, and light punctuation, so a case tests
/// recognizing a food rather than surviving parser junk.
fn plain_name(name: &str) -> bool {
    name.len() <= 40
        && name.starts_with(|c: char| c.is_alphabetic())
        && name
            .chars()
            .all(|c| c.is_alphabetic() || matches!(c, ' ' | '-' | ',' | '\''))
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
        .filter(|name| plain_name(name))
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
    ] {
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
    let list: List = reqwest::get("https://openrouter.ai/api/v1/models")
        .await?
        .error_for_status()?
        .json()
        .await
        .context("Failed to read OpenRouter's model list")?;
    let mut prices = HashMap::new();
    for model in models {
        let Some(found) = list.data.iter().find(|m| &m.id == model) else {
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
}

impl Spend {
    fn add(&mut self, usage: &Usage) {
        self.calls += 1;
        self.prompt_tokens += u64::from(usage.prompt_tokens);
        self.completion_tokens += u64::from(usage.completion_tokens);
    }

    fn dollars(&self, price: &Price) -> f64 {
        self.prompt_tokens as f64 * price.prompt + self.completion_tokens as f64 * price.completion
    }
}

fn client_for(model: &str) -> Result<CachingAiClient> {
    // The ingredient calls' production settings (their timeout), with the
    // candidate model in place of the ingredient model.
    let mut config = AiConfig::from_env()
        .context("AI is not configured (OPENROUTER_API_KEY)")?
        .for_ingredients();
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

/// Ask `items` `batch_size` at a time, retrying a rejected batch (invalid, or
/// truncated) the way the production worker does; a rejected single item is
/// invalid, with no second chance, as in production. Provider and
/// configuration errors stop the run.
async fn in_batches<I: Clone, A>(
    items: &[I],
    batch_size: usize,
    split: Split,
    spend: &mut Spend,
    mut ask: impl AsyncFnMut(&[I]) -> Result<(A, Usage), AiError>,
    mut take: impl FnMut(A),
) -> Result<usize> {
    let mut invalid = 0;
    let mut queue: VecDeque<Vec<I>> = items.chunks(batch_size).map(<[I]>::to_vec).collect();
    while let Some(batch) = queue.pop_front() {
        match ask(&batch).await {
            Ok((answers, usage)) => {
                spend.add(&usage);
                take(answers);
            }
            Err(e) if e.is_answer_specific() => {
                spend.rejected_calls += 1;
                spend.truncated += usize::from(matches!(e, AiError::Truncated(_)));
                if batch.len() == 1 {
                    invalid += 1;
                    if spend.rejections.len() < 5 {
                        spend
                            .rejections
                            .push(e.to_string().chars().take(300).collect());
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
            Err(e) => return Err(e.into()),
        }
    }
    Ok(invalid)
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
    let client = client_for(model)?;
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
            estimate_ingredient_weights(&client, batch)
                .await
                .map(|result| (result.weights, result.usage))
        },
        |weights| answers.extend(weights),
    )
    .await?;
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
    let client = client_for(model)?;
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
            resolve_ingredient_names(&client, &batch)
                .await
                .map(|result| (result.resolutions, result.usage))
        },
        |resolutions| answers.extend(resolutions),
    )
    .await?;
    let (mut kcal, mut cups, mut pieces) = (Vec::new(), Vec::new(), Vec::new());
    let (mut cup_cases, mut piece_cases, mut unknown) = (0, 0, 0);
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
            within(&pieces, 0.25, piece_cases),
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
    let client = client_for(model)?;
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
            resolve_ingredient_names(&client, &batch)
                .await
                .map(|result| (result.resolutions, result.usage))
        },
        |resolutions| answers.extend(resolutions),
    )
    .await?;
    let (mut foods, mut not_foods) = (0, 0);
    let (mut right_food, mut wrong_food, mut unknown, mut right_not_food) = (0, 0, 0, 0);
    let mut misses = Vec::new();
    for case in cases {
        let answer = answers.get(&case.name);
        match &case.expected_entry {
            Some(expected) => {
                foods += 1;
                match answer {
                    Some(NameResolution::Entry(key))
                        if entry_id(key).as_ref() == Some(expected) =>
                    {
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
}

fn spec(suite: &str) -> SuiteSpec {
    match suite {
        "ingredient-weights" => SuiteSpec {
            title: "Ingredient weights",
            about: "Grams in one unit of a catalog food (`estimate_ingredient_weights`), against USDA: 60 densities (cup) and 60 piece weights in estimable units. Error is |estimate − USDA| / USDA; null means the model said there's no typical weight.",
            columns: &["Within 20%", "Within 50%", "Median error", "P90 error", "Null", "Invalid"],
        },
        "food-estimates" => SuiteSpec {
            title: "Food estimates",
            about: "Calories (and cup and piece weights) for a food with no catalog candidates (`resolve_ingredient_names`, answer \"estimate\"), against USDA for 80 foods given by their USDA description. Calorie error is against at least 20 kcal/100 g, so near-zero foods don't dominate. Cup and piece columns count foods where USDA has the weight; the piece is USDA's default portion, which isn't always a whole item.",
            columns: &["kcal within 20%", "kcal median error", "kcal P90 error", "Cup within 25%", "Piece within 25%", "Unknown", "Invalid"],
        },
        "ingredient-names" => SuiteSpec {
            title: "Ingredient names",
            about: "Picking the catalog food a name means from its candidates (`resolve_ingredient_names`), for 100 curated aliases (correct if the answer resolves to the alias's food; the alias itself is not offered) and 30 curated not-food names. Some curated aliases are judgment calls (delicata squash counts as acorn squash), so a sensible answer can score as wrong: compare models with each other rather than reading this as absolute accuracy.",
            columns: &["Correct food", "Wrong food", "Unknown", "Not food right", "Invalid"],
        },
        other => unreachable!("unknown suite {other}"),
    }
}

fn render(
    suite: &str,
    cases: usize,
    batch_size: usize,
    rows: &[(ModelResult, f64, usize, usize, Vec<String>)],
) -> String {
    let spec = spec(suite);
    let mut out = format!(
        "# AI eval: {}\n\nWritten by `make ai-eval`. {}\n\nGolden set: `{GOLDEN_DIR}/{suite}.json` ({cases} cases), asked {batch_size} per call (production asks {BATCH}). Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated); a rejected batch is retried as the production worker does (in halves for weights, item by item for names), and a rejected single item is invalid. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.\n\n",
        spec.title, spec.about
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
        "all" => SUITES.to_vec(),
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
                _ => {
                    let golden: Vec<NameCase> = read_json(&path)?;
                    cases = golden.len();
                    eval_names(model, &golden, batch_size, &mut spend).await
                }
            };
            // A failing model (a timeout) stops the run before any report is
            // overwritten; everything already answered is cached, so a rerun
            // without it (or after fixing the cause) is nearly free.
            let result = outcome.with_context(|| format!("{suite} / {model} failed"))?;
            let dollars = spend.dollars(&prices[model]);
            tracing::info!(suite, model, calls = spend.calls, dollars, "evaluated");
            rows.push((
                result,
                dollars,
                spend.rejected_calls,
                spend.truncated,
                spend.rejections,
            ));
        }
        // Production's batch size is the main report; others sit beside it.
        let name = if batch_size == BATCH {
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
