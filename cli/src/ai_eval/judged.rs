//! The human-judged suites: auto-tag, title normalization, description
//! generation, custom enrich and recipe photos, whose answers no rule can
//! score.
//!
//! A run writes a blind judging page per suite
//! (`logs/ai-evals/<suite>/judge.html`): each case's source and the distinct
//! answers nobody has judged yet, shuffled, with no model names. Its download
//! is the suite's judgments file (`data/ai-evals/judgments/<suite>.json`),
//! committed beside the golden set. Free-text answers are judged good, best or
//! bad, keyed by a hash of the answer, so a model that gives an answer already
//! judged reuses the judgment and only new answers reach the page. Tags are
//! judged once per recipe, as the tags that apply, and every model is scored
//! against that set.

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine as _;
use ramekin_core::ai::prompts::render_generate_recipe_photo_prompt;
use ramekin_core::ai::{
    custom_enrich, generate_description, generate_recipe_photo, normalize_title, suggest_tags,
    AiConfig, AiError, Usage,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::{
    client_for, dump, in_batches, pct, read_json, sample, worst, ModelResult, Spend, Split,
    SNAPSHOTS_DIR,
};

pub const JUDGMENTS_DIR: &str = "data/ai-evals/judgments";

/// A user's tags, as auto-tag picks from: the kinds of tag people keep.
pub const TAG_VOCABULARY: &[&str] = &[
    "Breakfast",
    "Lunch",
    "Dinner",
    "Dessert",
    "Snack",
    "Appetizer",
    "Side Dish",
    "Drinks",
    "Soup",
    "Salad",
    "Sandwich",
    "Pasta",
    "Bread",
    "Baking",
    "Vegetarian",
    "Vegan",
    "Gluten-Free",
    "Dairy-Free",
    "Chicken",
    "Beef",
    "Pork",
    "Seafood",
    "Eggs",
    "Weeknight",
    "Make-Ahead",
    "One-Pot",
    "Grilling",
    "Holiday",
    "Comfort Food",
    "Italian",
    "Mexican",
    "Indian",
    "Japanese",
];

/// Custom enrich cases: a snapshot and an instruction that suits it.
const ENRICH_CASES: [(&str, &str); 10] = [
    (
        "americastestkitchen-com_recipes-5027-best-beef-stew",
        "Make this vegetarian.",
    ),
    (
        "smittenkitchen-com_2015-04-salted-chocolate-chunk-cookies",
        "Halve the recipe.",
    ),
    (
        "americastestkitchen-com_recipes-11519-indian-butter-chicken-murgh-makhani",
        "Make it dairy-free.",
    ),
    (
        "smittenkitchen-com_2016-10-pumpkin-bread",
        "Convert the measurements to metric (grams and milliliters).",
    ),
    (
        "smittenkitchen-com_2019-03-perfect-meatballs-and-spaghetti",
        "Make it gluten-free.",
    ),
    (
        "americastestkitchen-com_recipes-6682-weeknight-roast-chicken",
        "Add a note on what I can do ahead of time.",
    ),
    (
        "cooking-nytimes-com_recipes-1020129-baked-ziti-with-sausage-meatballs-and-spinach",
        "Scale it to feed 12.",
    ),
    (
        "smittenkitchen-com_2010-02-chana-masala",
        "Make it mild enough for kids.",
    ),
    (
        "cooking-nytimes-com_recipes-1022111-fried-fish-sandwich",
        "Adapt it for an air fryer.",
    ),
    (
        "smittenkitchen-com_2007-11-curried-lentils-and-sweet-potatoes",
        "Split the instructions into short numbered steps.",
    ),
];

/// Recipe photo cases: dishes whose real look is crisp, structured or
/// colorful, so a model that renders everything as brown mush (or noodles as
/// worms) is caught. Soft brown dishes (risotto, pâté, a braise) look like
/// that anyway, so they can't tell models apart.
const PHOTO_CASES: [&str; 10] = [
    "seriouseats-com_real-texas-nachos-recipe",
    "smittenkitchen-com_2015-09-oat-and-wheat-sandwich-bread",
    "smittenkitchen-com_2013-02-italian-stuffed-cabbage",
    "smittenkitchen-com_2023-04-hash-brown-patties",
    "seriouseats-com_cheese-frenchee-recipe-11686537",
    "smittenkitchen-com_2015-08-takeout-style-sesame-noodles-with-cucumber",
    "cooking-nytimes-com_recipes-1019430-omurice-japanese-rice-omelet",
    "smittenkitchen-com_2016-05-chicken-gyro-salad",
    "smittenkitchen-com_2014-12-jelly-doughnuts",
    "americastestkitchen-com_recipes-14919-san-diego-fish-tacos",
];

/// Words of a decorated title, the kind title normalization should tidy.
const DECORATIONS: [&str; 10] = [
    "best",
    "easy",
    "easiest",
    "recipe",
    "perfect",
    "ultimate",
    "favorite",
    "foolproof",
    "my",
    "amazing",
];

// ---------------------------------------------------------------------------
// Golden sets

#[derive(Deserialize)]
struct Snapshot {
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    servings: Option<String>,
    instructions: String,
    #[serde(default)]
    source_url: Option<String>,
    #[serde(default)]
    source_name: Option<String>,
    ingredients: Vec<SnapshotIngredient>,
}

#[derive(Deserialize)]
struct SnapshotIngredient {
    item: String,
    #[serde(default)]
    measurements: Vec<Measurement>,
    #[serde(default)]
    note: Option<String>,
    raw: String,
    #[serde(default)]
    section: Option<String>,
}

/// A recipe as the features are given it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecipeCase {
    /// The snapshot's file name, without `.json`.
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    /// One line per ingredient, with a colon-terminated heading wherever the
    /// section changes.
    pub ingredients: Vec<String>,
    pub instructions: String,
}

impl RecipeCase {
    fn ingredient_text(&self) -> String {
        self.ingredients.join("\n")
    }
}

/// Mirrors the server's `RecipeContent` (server/src/types.rs), the shape
/// custom enrich must answer in: production parses the answer into it, so an
/// answer that doesn't fit is invalid here too.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnrichRecipe {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub ingredients: Vec<EnrichIngredient>,
    pub instructions: String,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub source_name: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub servings: Option<String>,
    #[serde(default)]
    pub prep_time: Option<String>,
    #[serde(default)]
    pub cook_time: Option<String>,
    #[serde(default)]
    pub total_time: Option<String>,
    #[serde(default)]
    pub rating: Option<i32>,
    #[serde(default)]
    pub difficulty: Option<String>,
    #[serde(default)]
    pub nutritional_info: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Mirrors the server's `Ingredient` (server/src/models.rs).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnrichIngredient {
    pub item: String,
    pub measurements: Vec<Measurement>,
    pub note: Option<String>,
    pub section: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Measurement {
    pub amount: Option<String>,
    pub unit: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnrichCase {
    pub id: String,
    pub instruction: String,
    pub recipe: EnrichRecipe,
}

fn ingredient_lines(ingredients: &[SnapshotIngredient]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut section = None;
    for ingredient in ingredients {
        if ingredient.section != section {
            if let Some(heading) = &ingredient.section {
                lines.push(format!("{heading}:"));
            }
            section = ingredient.section.clone();
        }
        lines.push(ingredient.raw.clone());
    }
    lines
}

fn recipe_case(id: &str, snapshot: &Snapshot) -> RecipeCase {
    RecipeCase {
        id: id.to_string(),
        // Some snapshots keep a trailing space or newline.
        title: snapshot.title.trim().to_string(),
        description: snapshot.description.clone(),
        ingredients: ingredient_lines(&snapshot.ingredients),
        instructions: snapshot.instructions.clone(),
    }
}

fn enrich_recipe(snapshot: Snapshot) -> EnrichRecipe {
    EnrichRecipe {
        title: snapshot.title.trim().to_string(),
        description: snapshot.description,
        ingredients: snapshot
            .ingredients
            .into_iter()
            .map(|i| EnrichIngredient {
                item: i.item,
                measurements: i.measurements,
                note: i.note,
                section: i.section,
            })
            .collect(),
        instructions: snapshot.instructions,
        source_url: snapshot.source_url,
        source_name: snapshot.source_name,
        tags: Vec::new(),
        servings: snapshot.servings,
        prep_time: None,
        cook_time: None,
        total_time: None,
        rating: None,
        difficulty: None,
        nutritional_info: None,
        notes: None,
    }
}

/// Whether a title carries the decoration title normalization removes:
/// praise, "recipe", a parenthetical, or a subtitle.
fn decorated(title: &str) -> bool {
    title.contains(['(', '!', ':', '|'])
        || title.contains(" - ")
        || title
            .split(|c: char| !c.is_alphanumeric() && c != '\'')
            .any(|word| DECORATIONS.contains(&word.to_lowercase().as_str()))
}

fn read_snapshots(root: &Path) -> Result<BTreeMap<String, Snapshot>> {
    let mut snapshots = BTreeMap::new();
    for entry in fs::read_dir(root.join(SNAPSHOTS_DIR))? {
        let path = entry?.path();
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("Bad snapshot path {}", path.display()))?
            .to_string();
        snapshots.insert(id, read_json(&path)?);
    }
    Ok(snapshots)
}

/// Each judged suite's golden set, as (suite, JSON).
pub fn write_golden(root: &Path) -> Result<Vec<(&'static str, String)>> {
    let mut snapshots = read_snapshots(root)?;
    let recipes: Vec<RecipeCase> = snapshots
        .iter()
        .filter(|(_, s)| {
            !s.title.trim().is_empty()
                && !s.instructions.trim().is_empty()
                && s.ingredients.len() >= 3
        })
        .map(|(id, s)| recipe_case(id, s))
        .collect();
    // Each suite samples with its own key, so they draw different recipes.
    let pick = |suite: &str, cases: Vec<RecipeCase>, n: usize| {
        sample(cases, |c| format!("{suite} {}", c.id), n)
    };
    let (decorated_titles, plain_titles): (Vec<_>, Vec<_>) =
        recipes.iter().cloned().partition(|c| decorated(&c.title));
    // Mostly titles to tidy, and some already tidy, which should stay as
    // they are.
    let mut titles = pick("titles", decorated_titles, 14);
    titles.extend(pick("titles", plain_titles, 6));
    let tags = pick("tags", recipes.clone(), 20);
    let descriptions = pick("descriptions", recipes.clone(), 15);
    let photos = PHOTO_CASES
        .iter()
        .map(|id| {
            let snapshot = snapshots
                .get(*id)
                .with_context(|| format!("No pipeline snapshot {id} for a recipe-photos case"))?;
            Ok(recipe_case(id, snapshot))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut enrich = Vec::new();
    for (id, instruction) in ENRICH_CASES {
        let snapshot = snapshots
            .remove(id)
            .with_context(|| format!("No pipeline snapshot {id} for a custom-enrich case"))?;
        enrich.push(EnrichCase {
            id: id.to_string(),
            instruction: instruction.to_string(),
            recipe: enrich_recipe(snapshot),
        });
    }

    Ok(vec![
        ("tags", serde_json::to_string_pretty(&tags)?),
        ("titles", serde_json::to_string_pretty(&titles)?),
        ("descriptions", serde_json::to_string_pretty(&descriptions)?),
        ("custom-enrich", serde_json::to_string_pretty(&enrich)?),
        ("recipe-photos", serde_json::to_string_pretty(&photos)?),
    ])
}

// ---------------------------------------------------------------------------
// Judgments

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Among the best answers to the case.
    Best,
    Good,
    Bad,
}

/// One judged answer. `answer` is the answer itself where it's short text
/// (titles, descriptions), so the file reads on its own.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Judgment {
    pub verdict: Verdict,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
}

/// Case id → answer key → judgment.
pub type Verdicts = BTreeMap<String, BTreeMap<String, Judgment>>;
/// Case id → the tags that apply.
pub type TagSets = BTreeMap<String, Vec<String>>;

fn judgments_path(root: &Path, suite: &str) -> PathBuf {
    root.join(JUDGMENTS_DIR).join(format!("{suite}.json"))
}

/// A suite's judgments, or none yet.
fn read_judgments<T: Default + serde::de::DeserializeOwned>(root: &Path, suite: &str) -> Result<T> {
    let path = judgments_path(root, suite);
    if path.exists() {
        read_json(path)
    } else {
        Ok(T::default())
    }
}

/// An answer's key: a short hash of its content.
pub fn answer_key(content: &[u8]) -> String {
    let digest = Sha256::digest(content);
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Answers

/// An answer as the judging page shows it.
#[derive(Clone, Debug, Serialize)]
pub struct Shown {
    pub key: String,
    /// The answer as text (titles, descriptions, an enriched recipe).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// An image's path, relative to the page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

/// A case on the judging page.
#[derive(Debug, Serialize)]
pub struct PageCase {
    pub id: String,
    /// The source shown above the answers.
    pub source: String,
    /// What the answers change (custom enrich), whose lines answers are
    /// compared to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
    pub answers: Vec<Shown>,
}

/// A recipe as plain text: the source on the page, and an enriched recipe's
/// answer (every field, so two answers differ in text whenever they differ).
fn recipe_text(recipe: &EnrichRecipe) -> String {
    let mut out = format!("{}\n", recipe.title);
    let mut field = |label: &str, value: Option<&str>| {
        if let Some(value) = value.filter(|v| !v.trim().is_empty()) {
            out.push_str(&format!("{label}: {}\n", value.trim()));
        }
    };
    field("Description", recipe.description.as_deref());
    field("Servings", recipe.servings.as_deref());
    field("Prep time", recipe.prep_time.as_deref());
    field("Cook time", recipe.cook_time.as_deref());
    field("Total time", recipe.total_time.as_deref());
    field("Difficulty", recipe.difficulty.as_deref());
    field("Nutrition", recipe.nutritional_info.as_deref());
    field("Source", recipe.source_name.as_deref());
    field("Source URL", recipe.source_url.as_deref());
    let rating = recipe.rating.map(|r| r.to_string());
    field("Rating", rating.as_deref());
    let tags = recipe.tags.join(", ");
    field("Tags", Some(&tags));
    out.push_str("\nIngredients\n");
    let mut section = None;
    for ingredient in &recipe.ingredients {
        if ingredient.section != section {
            if let Some(heading) = &ingredient.section {
                out.push_str(&format!("{heading}:\n"));
            }
            section = ingredient.section.clone();
        }
        // The first measurement leads; alternatives follow in parentheses,
        // as "2 medium (2 tsp) garlic cloves".
        let measurements: Vec<String> = ingredient
            .measurements
            .iter()
            .map(|m| {
                [m.amount.as_deref(), m.unit.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|m| !m.is_empty())
            .collect();
        let mut line = match measurements.split_first() {
            None => ingredient.item.clone(),
            Some((first, [])) => format!("{first} {}", ingredient.item),
            Some((first, rest)) => format!("{first} ({}) {}", rest.join(", "), ingredient.item),
        };
        if let Some(note) = ingredient.note.as_deref().filter(|n| !n.is_empty()) {
            line.push_str(&format!(", {note}"));
        }
        out.push_str(&format!("{line}\n"));
    }
    out.push_str(&format!("\nInstructions\n{}\n", recipe.instructions.trim()));
    if let Some(notes) = recipe.notes.as_deref().filter(|n| !n.trim().is_empty()) {
        out.push_str(&format!("\nNotes\n{}\n", notes.trim()));
    }
    out
}

fn source_text(case: &RecipeCase, with_description: bool) -> String {
    let mut out = format!("{}\n", case.title);
    if with_description {
        if let Some(description) = case.description.as_deref().filter(|d| !d.is_empty()) {
            out.push_str(&format!("{description}\n"));
        }
    }
    out.push_str(&format!(
        "\nIngredients\n{}\n\nInstructions\n{}\n",
        case.ingredient_text(),
        case.instructions.trim()
    ));
    out
}

/// A photo model named as `model@quality`: the OpenRouter id and the quality
/// to ask for.
pub fn split_quality(model: &str) -> (&str, Option<&str>) {
    match model.split_once('@') {
        Some((id, quality)) => (id, Some(quality)),
        None => (model, None),
    }
}

/// A generated photo, as cached: the provider's data URL and reported cost.
#[derive(Serialize, Deserialize)]
struct CachedPhoto {
    data_url: String,
    cost: Option<f64>,
}

/// A photo's bytes and file extension, from a data URL.
fn decode_data_url(url: &str) -> Result<(Vec<u8>, &'static str), AiError> {
    let bad = || AiError::ParseError(format!("Not an image data URL: {:.60}", url));
    let (header, data) = url
        .strip_prefix("data:")
        .and_then(|rest| rest.split_once(','))
        .ok_or_else(bad)?;
    let ext = match header.split(';').next() {
        Some("image/png") => "png",
        Some("image/jpeg") => "jpg",
        Some("image/webp") => "webp",
        _ => return Err(bad()),
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| bad())?;
    Ok((bytes, ext))
}

/// Generates a case's photo through production's call, cached under the AI
/// cache directory by model and prompt: the provider doesn't cache it, and
/// each costs a few cents.
async fn photo(config: &AiConfig, case: &RecipeCase) -> Result<CachedPhoto, AiError> {
    let ingredients = case.ingredient_text();
    let prompt = render_generate_recipe_photo_prompt(
        &case.title,
        case.description.as_deref(),
        &ingredients,
        &case.instructions,
    );
    let path = config
        .namespaced_cache_dir()
        .join("recipe-photos")
        .join(match &config.image_quality {
            Some(quality) => format!("{}@{quality}", config.image_model.replace('/', "_")),
            None => config.image_model.replace('/', "_"),
        })
        .join(format!("{}.json", answer_key(prompt.as_bytes())));
    if let Some(cached) = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<CachedPhoto>(&text).ok())
    {
        return Ok(cached);
    }
    let result = generate_recipe_photo(
        config,
        &case.title,
        case.description.as_deref(),
        &ingredients,
        &case.instructions,
    )
    .await?;
    decode_data_url(&result.image_data_url)?;
    let cached = CachedPhoto {
        data_url: result.image_data_url,
        cost: result.cost,
    };
    let write = || -> std::io::Result<()> {
        fs::create_dir_all(path.parent().expect("cache path has a parent"))?;
        fs::write(&path, serde_json::to_string(&cached)?)
    };
    if let Err(e) = write() {
        tracing::warn!("Failed to cache {}: {e}", path.display());
    }
    Ok(cached)
}

/// Asks every case once, as production does (no retry), and dumps the
/// answers. Returns each case's answer (None if invalid) and the invalid
/// count.
async fn ask_all<C: Serialize, A: Clone + Serialize>(
    suite: &str,
    model: &str,
    cases: &[C],
    spend: &mut Spend,
    mut ask: impl AsyncFnMut(usize) -> Result<(A, Usage), AiError>,
) -> Result<(Vec<Option<A>>, usize)> {
    let indexes: Vec<usize> = (0..cases.len()).collect();
    let mut answers = vec![None; cases.len()];
    let invalid = in_batches(
        &indexes,
        1,
        Split::Singles,
        spend,
        async |batch: &[usize]| {
            let i = batch[0];
            ask(i).await.map(|(answer, usage)| ((i, answer), usage))
        },
        |(i, answer)| answers[i] = Some(answer),
    )
    .await
    .with_context(|| format!("{suite} / {model} failed"))?;
    dump(
        suite,
        model,
        cases
            .iter()
            .zip(&answers)
            .map(|(case, answer)| serde_json::json!({ "case": case, "answer": answer }))
            .collect(),
    )?;
    Ok((answers, invalid))
}

// ---------------------------------------------------------------------------
// Scoring

pub const VERDICT_COLUMNS: &[&str] = &["Good", "Best", "Unjudged", "Invalid"];
pub const TAG_COLUMNS: &[&str] = &["Exact", "Precision", "Recall", "Unjudged", "Invalid"];

/// One model's answers to a verdict suite: per case, the shown answer (None
/// if invalid).
fn score_verdicts(
    model: &str,
    case_ids: &[String],
    answers: &[Option<Shown>],
    invalid: usize,
    verdicts: &Verdicts,
) -> ModelResult {
    let (mut judged, mut good, mut best, mut unjudged) = (0, 0, 0, 0);
    let mut misses = Vec::new();
    for (id, answer) in case_ids.iter().zip(answers) {
        let Some(answer) = answer else { continue };
        match verdicts.get(id).and_then(|v| v.get(&answer.key)) {
            None => unjudged += 1,
            Some(judgment) => {
                judged += 1;
                match judgment.verdict {
                    Verdict::Best => {
                        best += 1;
                        good += 1;
                    }
                    Verdict::Good => good += 1,
                    Verdict::Bad => {
                        let shown = answer
                            .text
                            .as_deref()
                            .map(|t| format!("{:?}", t.chars().take(120).collect::<String>()))
                            .unwrap_or_else(|| format!("answer {}", answer.key));
                        misses.push((1.0, format!("{id}: judged bad: {shown}")));
                    }
                }
            }
        }
    }
    ModelResult {
        model: model.into(),
        cells: vec![
            pct(good, judged),
            pct(best, judged),
            unjudged.to_string(),
            pct(invalid, case_ids.len()),
        ],
        misses: worst(misses),
    }
}

/// One model's tags against the judged tag sets, compared case-insensitively.
fn score_tags(
    model: &str,
    case_ids: &[String],
    answers: &[Option<Vec<String>>],
    invalid: usize,
    tag_sets: &TagSets,
) -> ModelResult {
    let lower =
        |tags: &[String]| -> BTreeSet<String> { tags.iter().map(|t| t.to_lowercase()).collect() };
    let (mut judged, mut exact, mut right, mut answered, mut expected, mut unjudged) =
        (0, 0, 0, 0, 0, 0);
    let mut misses = Vec::new();
    for (id, answer) in case_ids.iter().zip(answers) {
        let Some(answer) = answer else { continue };
        let Some(truth) = tag_sets.get(id) else {
            unjudged += 1;
            continue;
        };
        let (answer, truth) = (lower(answer), lower(truth));
        judged += 1;
        right += answer.intersection(&truth).count();
        answered += answer.len();
        expected += truth.len();
        if answer == truth {
            exact += 1;
        } else {
            let missed: Vec<_> = truth.difference(&answer).collect();
            let extra: Vec<_> = answer.difference(&truth).collect();
            misses.push((
                (missed.len() + extra.len()) as f64,
                format!("{id}: missed {missed:?}, extra {extra:?}"),
            ));
        }
    }
    ModelResult {
        model: model.into(),
        cells: vec![
            pct(exact, judged),
            pct(right, answered),
            pct(right, expected),
            unjudged.to_string(),
            pct(invalid, case_ids.len()),
        ],
        misses: worst(misses),
    }
}

// ---------------------------------------------------------------------------
// Running a suite

/// The answers no judgment covers yet, gathered across models for the page:
/// per case, distinct by key.
#[derive(Default)]
struct Unjudged {
    by_case: BTreeMap<String, BTreeMap<String, Shown>>,
}

impl Unjudged {
    fn add(&mut self, case: &str, verdicts: &Verdicts, answer: &Shown) {
        if verdicts
            .get(case)
            .is_some_and(|v| v.contains_key(&answer.key))
        {
            return;
        }
        self.by_case
            .entry(case.to_string())
            .or_default()
            .entry(answer.key.clone())
            .or_insert_with(|| answer.clone());
    }

    /// A case's unjudged answers in a stable shuffle: by hash of case and
    /// key, so no position says which model answered.
    fn take(&mut self, case: &str) -> Vec<Shown> {
        let mut answers: Vec<Shown> = self
            .by_case
            .remove(case)
            .map(|m| m.into_values().collect())
            .unwrap_or_default();
        answers.sort_by_cached_key(|a| Sha256::digest(format!("{case} {}", a.key).as_bytes()));
        answers
    }
}

/// What a run of one judged suite produces: one result per model, with its
/// spend.
pub struct SuiteRun {
    pub cases: usize,
    pub results: Vec<(ModelResult, Spend)>,
}

pub async fn eval(root: &Path, suite: &str, models: &[String]) -> Result<SuiteRun> {
    let golden = root.join(super::GOLDEN_DIR).join(format!("{suite}.json"));
    let page_dir = Path::new("logs/ai-evals").join(suite);
    let mut results = Vec::new();
    let mut page = Vec::new();
    // Each model's answers, for the unblinded results page.
    let mut answered: Vec<(String, Vec<Option<Shown>>)> = Vec::new();
    let cases;
    match suite {
        "tags" => {
            let golden: Vec<RecipeCase> = read_json(&golden)?;
            cases = golden.len();
            let tag_sets: TagSets = read_judgments(root, suite)?;
            let vocabulary: Vec<String> = TAG_VOCABULARY.iter().map(|t| t.to_string()).collect();
            let ids: Vec<String> = golden.iter().map(|c| c.id.clone()).collect();
            for model in models {
                let client = client_for(model, AiConfig::clone)?;
                let mut spend = Spend::default();
                let (answers, invalid) = ask_all(suite, model, &golden, &mut spend, async |i| {
                    let case = &golden[i];
                    suggest_tags(
                        &client,
                        &case.title,
                        // As the server sends them: one line, comma-separated.
                        &case.ingredients.join(", "),
                        &case.instructions,
                        &vocabulary,
                    )
                    .await
                    .map(|r| (r.suggested_tags, r.usage))
                })
                .await?;
                results.push((score_tags(model, &ids, &answers, invalid, &tag_sets), spend));
            }
            // Tags are judged once per recipe, not per answer: the page asks
            // for the recipes with no tag set yet.
            page = golden
                .iter()
                .filter(|c| !tag_sets.contains_key(&c.id))
                .map(|c| PageCase {
                    id: c.id.clone(),
                    source: source_text(c, true),
                    original: None,
                    instruction: None,
                    answers: Vec::new(),
                })
                .collect();
            write_page(&page_dir, suite, &page, &tag_sets)?;
        }
        "titles" | "descriptions" => {
            let golden: Vec<RecipeCase> = read_json(&golden)?;
            cases = golden.len();
            let verdicts: Verdicts = read_judgments(root, suite)?;
            let ids: Vec<String> = golden.iter().map(|c| c.id.clone()).collect();
            let mut unjudged = Unjudged::default();
            for model in models {
                let client = client_for(model, AiConfig::clone)?;
                let mut spend = Spend::default();
                let (answers, invalid) = ask_all(suite, model, &golden, &mut spend, async |i| {
                    let case = &golden[i];
                    let ingredients = case.ingredient_text();
                    if suite == "titles" {
                        normalize_title(&client, &case.title, &ingredients, &case.instructions)
                            .await
                            .map(|r| (r.normalized_title.trim().to_string(), r.usage))
                    } else {
                        generate_description(&client, &case.title, &ingredients, &case.instructions)
                            .await
                            .map(|r| (r.description.trim().to_string(), r.usage))
                    }
                })
                .await?;
                let shown: Vec<Option<Shown>> = answers
                    .into_iter()
                    .map(|a| {
                        a.map(|text| Shown {
                            key: answer_key(text.as_bytes()),
                            text: Some(text),
                            image: None,
                        })
                    })
                    .collect();
                for (id, answer) in ids.iter().zip(&shown) {
                    if let Some(answer) = answer {
                        unjudged.add(id, &verdicts, answer);
                    }
                }
                answered.push((model.clone(), shown.clone()));
                results.push((
                    score_verdicts(model, &ids, &shown, invalid, &verdicts),
                    spend,
                ));
            }
            for case in &golden {
                let answers = unjudged.take(&case.id);
                if !answers.is_empty() {
                    page.push(PageCase {
                        id: case.id.clone(),
                        // A description is what's being written, so the page
                        // leaves the original out.
                        source: source_text(case, false),
                        original: None,
                        instruction: None,
                        answers,
                    });
                }
            }
            write_page(&page_dir, suite, &page, &verdicts)?;
            write_results(
                &page_dir,
                suite,
                &golden
                    .iter()
                    .map(|c| (c.id.clone(), c.title.clone()))
                    .collect::<Vec<_>>(),
                &answered,
                &verdicts,
            )?;
        }
        "custom-enrich" => {
            let golden: Vec<EnrichCase> = read_json(&golden)?;
            cases = golden.len();
            let verdicts: Verdicts = read_judgments(root, suite)?;
            let ids: Vec<String> = golden.iter().map(|c| c.id.clone()).collect();
            let mut unjudged = Unjudged::default();
            for model in models {
                let client = client_for(model, AiConfig::clone)?;
                let mut spend = Spend::default();
                let (answers, invalid) = ask_all(suite, model, &golden, &mut spend, async |i| {
                    let case = &golden[i];
                    // As the server sends it.
                    let recipe =
                        serde_json::to_string_pretty(&case.recipe).expect("a recipe serializes");
                    custom_enrich::<EnrichRecipe>(&client, &recipe, &case.instruction, vec![])
                        .await
                        .map(|r| (r.recipe, r.usage))
                })
                .await?;
                let shown: Vec<Option<Shown>> = answers
                    .iter()
                    .map(|a| {
                        a.as_ref().map(|recipe| {
                            let text = recipe_text(recipe);
                            Shown {
                                key: answer_key(text.as_bytes()),
                                text: Some(text),
                                image: None,
                            }
                        })
                    })
                    .collect();
                for (id, answer) in ids.iter().zip(&shown) {
                    if let Some(answer) = answer {
                        unjudged.add(id, &verdicts, answer);
                    }
                }
                answered.push((model.clone(), shown.clone()));
                results.push((
                    score_verdicts(model, &ids, &shown, invalid, &verdicts),
                    spend,
                ));
            }
            for case in &golden {
                let answers = unjudged.take(&case.id);
                if !answers.is_empty() {
                    let original = recipe_text(&case.recipe);
                    page.push(PageCase {
                        id: case.id.clone(),
                        source: original.clone(),
                        original: Some(original),
                        instruction: Some(case.instruction.clone()),
                        answers,
                    });
                }
            }
            write_page(&page_dir, suite, &page, &verdicts)?;
            write_results(
                &page_dir,
                suite,
                &golden
                    .iter()
                    .map(|c| {
                        (
                            c.id.clone(),
                            format!("{}: {}", c.recipe.title, c.instruction),
                        )
                    })
                    .collect::<Vec<_>>(),
                &answered,
                &verdicts,
            )?;
        }
        "recipe-photos" => {
            let golden: Vec<RecipeCase> = read_json(&golden)?;
            cases = golden.len();
            let verdicts: Verdicts = read_judgments(root, suite)?;
            let ids: Vec<String> = golden.iter().map(|c| c.id.clone()).collect();
            let mut unjudged = Unjudged::default();
            let image_dir = page_dir.join("images");
            fs::create_dir_all(&image_dir)?;
            for model in models {
                let mut config =
                    AiConfig::from_env().context("AI is not configured (OPENROUTER_API_KEY)")?;
                // `model@high` asks for that image quality; plain, the
                // model's own default.
                let (id, quality) = split_quality(model);
                config.image_model = id.to_string();
                config.image_quality = quality.map(str::to_string);
                let mut spend = Spend::default();
                // Image tokens are priced apart from text, so the cost is
                // what the provider reports.
                let mut cost = 0.0;
                // Written after asking, so a local write failure stops the
                // run instead of counting as the model's answer.
                let mut images = Vec::new();
                let (answers, invalid) = ask_all(suite, model, &golden, &mut spend, async |i| {
                    let cached = photo(&config, &golden[i]).await?;
                    let (bytes, ext) = decode_data_url(&cached.data_url)?;
                    let key = answer_key(&bytes);
                    let file = format!("{key}.{ext}");
                    images.push((file.clone(), bytes));
                    cost += cached.cost.unwrap_or(0.0);
                    Ok((
                        Shown {
                            key,
                            text: None,
                            image: Some(format!("images/{file}")),
                        },
                        Usage::default(),
                    ))
                })
                .await?;
                for (file, bytes) in images {
                    let path = image_dir.join(file);
                    fs::write(&path, bytes)
                        .with_context(|| format!("Failed to write {}", path.display()))?;
                }
                spend.reported_dollars += cost;
                for (id, answer) in ids.iter().zip(&answers) {
                    if let Some(answer) = answer {
                        unjudged.add(id, &verdicts, answer);
                    }
                }
                answered.push((model.clone(), answers.clone()));
                results.push((
                    score_verdicts(model, &ids, &answers, invalid, &verdicts),
                    spend,
                ));
            }
            for case in &golden {
                let answers = unjudged.take(&case.id);
                if !answers.is_empty() {
                    page.push(PageCase {
                        id: case.id.clone(),
                        source: source_text(case, true),
                        original: None,
                        instruction: None,
                        answers,
                    });
                }
            }
            write_page(&page_dir, suite, &page, &verdicts)?;
            write_results(
                &page_dir,
                suite,
                &golden
                    .iter()
                    .map(|c| (c.id.clone(), c.title.clone()))
                    .collect::<Vec<_>>(),
                &answered,
                &verdicts,
            )?;
        }
        other => bail!("{other} is not a judged suite"),
    }
    Ok(SuiteRun { cases, results })
}

// ---------------------------------------------------------------------------
// Judging page

const PAGE: &str = include_str!("judge.html");
const RESULTS: &str = include_str!("results.html");

/// Writes `<dir>/results.html`: every model's answer to every case, named
/// and with its judgment, for looking at one model's answers after judging.
fn write_results(
    dir: &Path,
    suite: &str,
    cases: &[(String, String)],
    answered: &[(String, Vec<Option<Shown>>)],
    verdicts: &Verdicts,
) -> Result<()> {
    fs::create_dir_all(dir)?;
    let rows: Vec<serde_json::Value> = cases
        .iter()
        .enumerate()
        .map(|(i, (id, title))| {
            let answers: Vec<serde_json::Value> = answered
                .iter()
                .map(|(model, answers)| {
                    let answer = answers[i].as_ref();
                    let verdict = answer
                        .and_then(|a| verdicts.get(id).and_then(|v| v.get(&a.key)))
                        .map(|j| j.verdict);
                    serde_json::json!({ "model": model, "answer": answer, "verdict": verdict })
                })
                .collect();
            serde_json::json!({ "id": id, "title": title, "answers": answers })
        })
        .collect();
    let models: Vec<&str> = answered.iter().map(|(m, _)| m.as_str()).collect();
    let data = serde_json::json!({ "suite": suite, "models": models, "cases": rows });
    let data = serde_json::to_string(&data)?.replace("</", "<\\/");
    let path = dir.join("results.html");
    fs::write(&path, RESULTS.replace("__DATA__", &data))
        .with_context(|| format!("Failed to write {}", path.display()))
}

/// Writes `<dir>/judge.html`: the cases to judge, and the suite's judgments
/// so far, which its download merges the new ones into.
fn write_page<T: Serialize>(
    dir: &Path,
    suite: &str,
    cases: &[PageCase],
    existing: &T,
) -> Result<()> {
    fs::create_dir_all(dir)?;
    let data = serde_json::json!({
        "suite": suite,
        "kind": if suite == "tags" { "tags" } else { "verdicts" },
        // Short answers are kept in the judgments file, so it reads on its
        // own; a recipe or an image is too long.
        "keepText": matches!(suite, "titles" | "descriptions"),
        "vocabulary": TAG_VOCABULARY,
        "existing": existing,
        "cases": cases,
    });
    // `</` would end the script element the data sits in.
    let data = serde_json::to_string(&data)?.replace("</", "<\\/");
    let path = dir.join("judge.html");
    fs::write(&path, PAGE.replace("__DATA__", &data))
        .with_context(|| format!("Failed to write {}", path.display()))?;
    if cases.is_empty() {
        tracing::info!(suite, "Every answer is judged");
    } else {
        tracing::info!(
            suite,
            cases = cases.len(),
            "Wrote {}: judge it and save the download as {JUDGMENTS_DIR}/{suite}.json",
            path.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(text: &str) -> Shown {
        Shown {
            key: answer_key(text.as_bytes()),
            text: Some(text.to_string()),
            image: None,
        }
    }

    fn judged(text: &str, verdict: Verdict) -> (String, Judgment) {
        (
            answer_key(text.as_bytes()),
            Judgment {
                verdict,
                answer: Some(text.to_string()),
            },
        )
    }

    #[test]
    fn decorated_titles() {
        assert!(decorated("The Best Crispy Roast Potatoes Ever"));
        assert!(decorated("Classic Guacamole Recipe"));
        assert!(decorated("Pad See Ew (Thai Stir-Fried Noodles)"));
        assert!(decorated("My Mother's Peasant Bread: Easy"));
        assert!(!decorated("Pad Thai"));
        assert!(!decorated("Skirt Steak Salad with Arugula and Blue Cheese"));
        // A word, not a prefix of one.
        assert!(!decorated("Bestia's Mussels"));
    }

    #[test]
    fn verdicts_count_judged_answers_and_leave_the_rest_unjudged() {
        let ids: Vec<String> = ["a", "b", "c", "d"].map(String::from).to_vec();
        let verdicts: Verdicts = [
            ("a".to_string(), [judged("Pad Thai", Verdict::Best)].into()),
            ("b".to_string(), [judged("Bad Title", Verdict::Bad)].into()),
        ]
        .into();
        let answers = vec![
            Some(shown("Pad Thai")),
            Some(shown("Bad Title")),
            Some(shown("Never Judged")),
            None,
        ];
        let result = score_verdicts("m", &ids, &answers, 1, &verdicts);
        // Good 1 of 2 judged, best 1 of 2, 1 unjudged, 1 of 4 invalid.
        assert_eq!(result.cells, ["50%", "50%", "1", "25%"]);
        assert_eq!(result.misses.len(), 1);
        assert!(result.misses[0].1.contains("Bad Title"));
    }

    #[test]
    fn an_answer_judged_for_one_case_is_unjudged_for_another() {
        let ids: Vec<String> = vec!["b".into()];
        let verdicts: Verdicts = [("a".to_string(), [judged("Same", Verdict::Good)].into())].into();
        let result = score_verdicts("m", &ids, &[Some(shown("Same"))], 0, &verdicts);
        assert_eq!(result.cells, ["–", "–", "1", "0%"]);
    }

    #[test]
    fn tags_score_case_insensitively_against_the_judged_set() {
        let ids: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
        let tag_sets: TagSets = [
            ("a".to_string(), vec!["Dinner".into(), "Beef".into()]),
            ("b".to_string(), vec!["Dessert".into()]),
        ]
        .into();
        let answers = vec![
            Some(vec!["beef".into(), "Dinner".into()]),
            Some(vec!["Dessert".into(), "Baking".into()]),
            Some(vec!["Soup".into()]),
        ];
        let result = score_tags("m", &ids, &answers, 0, &tag_sets);
        // Exact 1 of 2; 3 right of 4 answered; 3 of 3 expected; c unjudged.
        assert_eq!(result.cells, ["50%", "75%", "100%", "1", "0%"]);
        assert_eq!(result.misses.len(), 1);
        assert!(result.misses[0].1.contains("baking"));
    }

    #[test]
    fn unjudged_answers_merge_across_models_and_skip_judged_ones() {
        let verdicts: Verdicts = [("a".to_string(), [judged("Old", Verdict::Good)].into())].into();
        let mut unjudged = Unjudged::default();
        for text in ["Old", "New", "New", "Other"] {
            unjudged.add("a", &verdicts, &shown(text));
        }
        let answers = unjudged.take("a");
        let mut texts: Vec<_> = answers.iter().map(|a| a.text.clone().unwrap()).collect();
        texts.sort();
        assert_eq!(texts, ["New", "Other"]);
        assert!(unjudged.take("a").is_empty());
    }

    #[test]
    fn photo_models_name_a_quality_after_an_at() {
        assert_eq!(
            split_quality("openai/gpt-image-2.5-flare@high"),
            ("openai/gpt-image-2.5-flare", Some("high"))
        );
        assert_eq!(split_quality("meta/muse-image"), ("meta/muse-image", None));
    }

    #[test]
    fn data_urls_decode_to_typed_bytes() {
        let (bytes, ext) = decode_data_url("data:image/png;base64,aGk=").unwrap();
        assert_eq!((bytes.as_slice(), ext), (b"hi".as_slice(), "png"));
        assert!(decode_data_url("data:text/plain;base64,aGk=").is_err());
        assert!(decode_data_url("https://example.com/a.png").is_err());
    }

    #[test]
    fn recipe_text_marks_sections_and_skips_empty_fields() {
        let recipe = EnrichRecipe {
            title: "Stew".into(),
            description: Some(" ".into()),
            ingredients: vec![EnrichIngredient {
                item: "beef".into(),
                measurements: vec![
                    Measurement {
                        amount: Some("2".into()),
                        unit: Some("lb".into()),
                    },
                    Measurement {
                        amount: Some("900".into()),
                        unit: Some("g".into()),
                    },
                ],
                note: Some("cubed".into()),
                section: Some("Stew".into()),
            }],
            instructions: "Brown it.".into(),
            source_url: None,
            source_name: None,
            tags: vec![],
            servings: Some("4".into()),
            prep_time: None,
            cook_time: None,
            total_time: None,
            rating: None,
            difficulty: None,
            nutritional_info: None,
            notes: None,
        };
        assert_eq!(
            recipe_text(&recipe),
            "Stew\nServings: 4\n\nIngredients\nStew:\n2 lb (900 g) beef, cubed\n\nInstructions\nBrown it.\n"
        );
    }

    #[test]
    fn page_data_cannot_close_its_script() {
        let dir = std::env::temp_dir().join(format!("judge-page-{}", std::process::id()));
        let case = PageCase {
            id: "x".into(),
            source: "</script><b>".into(),
            original: None,
            instruction: None,
            answers: vec![shown("ok")],
        };
        write_page(&dir, "titles", &[case], &Verdicts::new()).unwrap();
        let page = fs::read_to_string(dir.join("judge.html")).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert!(!page.contains("__DATA__"));
        assert_eq!(page.matches("</script>").count(), 2);
    }
}
