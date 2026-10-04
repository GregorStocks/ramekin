//! The extraction suites: a recipe read from pasted text
//! (`extract_recipe_from_text`) or from photos (`extract_recipe_from_photos`),
//! scored line by line against the recipe it should come out as.
//!
//! Text cases are pipeline snapshots rendered to the plain text a user would
//! paste (some wrapped in blog chatter, some with no description or servings),
//! plus a few texts that aren't recipes. Photo cases are hand-transcribed
//! photos of real pages in `data/ai-evals/photos/`.

use anyhow::{anyhow, Context, Result};
use base64::Engine as _;
use ramekin_core::ai::{
    extract_recipe_from_photos, text_extract::extract_recipe_from_text, AiConfig, AiError,
    ImageData, Usage,
};
use ramekin_core::ingredient_parser::{normalize_unicode, strip_leading_list_marker};
use ramekin_core::{validate_image, RawRecipe};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use super::{
    client_for, dump, in_batches, pct, read_json, sample, worst, ModelResult, Spend, Split,
};

pub const PHOTOS_DIR: &str = "data/ai-evals/photos";
const SNAPSHOTS_DIR: &str = "data/pipeline-snapshots";

/// The recipe an extraction should produce.
#[derive(Debug, Serialize, Deserialize)]
pub struct Expected {
    title: String,
    /// Ingredient lines, section headings as colon-terminated lines.
    ingredients: Vec<String>,
    instructions: String,
    /// Source text an answer may carry over without penalty (a variation, a
    /// headnote), in its instructions or notes.
    #[serde(default)]
    also_allowed: String,
    /// What the source states for these, for reading; an answer is scored
    /// only on not inventing numbers.
    servings: Option<String>,
    prep_time: Option<String>,
    cook_time: Option<String>,
    total_time: Option<String>,
}

impl Expected {
    /// Everything a photo case's page says, as text: what its fields
    /// transcribe.
    fn source(&self) -> String {
        let mut parts = vec![self.title.as_str(), &self.instructions, &self.also_allowed];
        parts.extend(self.ingredients.iter().map(String::as_str));
        parts.extend(
            [
                &self.servings,
                &self.prep_time,
                &self.cook_time,
                &self.total_time,
            ]
            .into_iter()
            .flatten()
            .map(String::as_str),
        );
        parts.join("\n")
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextCase {
    id: String,
    text: String,
    /// None when the text isn't a recipe and the fields should come back empty.
    expected: Option<Expected>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PhotoCase {
    id: String,
    /// Files in `PHOTOS_DIR`, sent in order as one request.
    photos: Vec<String>,
    /// What makes the photo hard, and any judgment call in its transcription.
    note: String,
    expected: Expected,
}

// ---------------------------------------------------------------------------
// Golden set (text only; the photo set is transcribed by hand)

#[derive(Deserialize)]
struct Snapshot {
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    servings: Option<String>,
    instructions: String,
    ingredients: Vec<SnapshotIngredient>,
}

#[derive(Deserialize)]
struct SnapshotIngredient {
    raw: String,
    section: Option<String>,
}

const CHATTER: [(&str, &str); 2] = [
    (
        "Posted by Jen · March 3 · 14 comments\n\nThis has been our family's favorite for years! The kids ask for it every single week, and it freezes beautifully. Scroll down for the printable recipe card.\n\nJump to Recipe · Print Recipe\n",
        "\nDid you make this? Leave a star rating and tag us on Instagram!\n\nYou might also like: Easy Weeknight Chili · The Best Banana Bread · 5-Minute Guacamole\n\n© 2025 Jen's Kitchen. All rights reserved.",
    ),
    (
        "Mom's recipe, copied from the card in her recipe box. She always made this for Sunday dinner.\n\n",
        "\nNote to self: double it next time, it went fast.",
    ),
];

const NOT_RECIPES: [(&str, &str); 3] = [
    (
        "shopping-list",
        "Shopping list for the week\n- milk\n- eggs (dozen)\n- 2 lb chicken thighs\n- baby spinach\n- coffee filters\n- dish soap\n- lemons x3",
    ),
    (
        "restaurant-review",
        "We finally tried the new ramen place on 5th Street. The tonkotsu broth was rich without being greasy, and the chashu was torched to order. Service was slow on a Friday night, and the gyoza came out after our bowls. Prices run about $18 a bowl. Worth a visit, but go early: there was a 40-minute wait by 7.",
    ),
    (
        "potluck-email",
        "Hi all, for Saturday's potluck we have mains and salads covered, so if you haven't signed up yet please bring a dessert or drinks. Plates and cutlery are taken care of. Doors open at 6, and parking is behind the community center. Thanks! — Priya",
    ),
];

/// Ingredient lines as the importer stores them: one per line, with a
/// colon-terminated heading wherever the section changes.
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

pub fn write_golden(root: &Path) -> Result<Vec<TextCase>> {
    let mut snapshots = Vec::new();
    for entry in fs::read_dir(root.join(SNAPSHOTS_DIR))? {
        let path = entry?.path();
        let snapshot: Snapshot = read_json(&path)?;
        if !snapshot.title.trim().is_empty()
            && !snapshot.instructions.trim().is_empty()
            && snapshot.ingredients.len() >= 3
        {
            snapshots.push(snapshot);
        }
    }
    let snapshots = sample(snapshots, |s| s.title.clone(), 30);
    let mut cases: Vec<TextCase> = snapshots
        .into_iter()
        .enumerate()
        .map(|(i, snapshot)| {
            // Every third case leaves out the description and servings, so a
            // model that fills them in is inventing them.
            let bare = i % 3 == 2;
            let chatter = (i % 3 == 1).then_some(CHATTER[i % 2]);
            let ingredients = ingredient_lines(&snapshot.ingredients);
            let description = snapshot.description.filter(|_| !bare);
            let servings = snapshot.servings.filter(|_| !bare);
            let mut text = String::new();
            if let Some((before, _)) = chatter {
                text.push_str(before);
                text.push('\n');
            }
            text.push_str(&snapshot.title);
            text.push_str("\n\n");
            if let Some(description) = &description {
                text.push_str(description);
                text.push_str("\n\n");
            }
            if let Some(servings) = &servings {
                // Some sites' servings already say what they are ("Makes 12").
                if servings.contains(char::is_alphabetic) {
                    text.push_str(&format!("{servings}\n\n"));
                } else {
                    text.push_str(&format!("Servings: {servings}\n\n"));
                }
            }
            text.push_str("Ingredients\n");
            text.push_str(&ingredients.join("\n"));
            text.push_str("\n\nInstructions\n");
            text.push_str(&snapshot.instructions);
            text.push('\n');
            if let Some((_, after)) = chatter {
                text.push_str(after);
                text.push('\n');
            }
            TextCase {
                id: snapshot.title.clone(),
                text,
                expected: Some(Expected {
                    title: snapshot.title,
                    ingredients,
                    instructions: snapshot.instructions,
                    also_allowed: description.unwrap_or_default(),
                    servings,
                    prep_time: None,
                    cook_time: None,
                    total_time: None,
                }),
            }
        })
        .collect();
    cases.extend(NOT_RECIPES.iter().map(|(id, text)| TextCase {
        id: (*id).into(),
        text: (*text).into(),
        expected: None,
    }));
    Ok(cases)
}

// ---------------------------------------------------------------------------
// Scoring

/// A line compared loosely: ASCII fractions and dashes (as the ingredient
/// parser reads them, so "1½" and "1 1/2" match), no list bullet, plain
/// quotes, lowercase, and single spaces.
fn normalize_line(line: &str) -> String {
    let line: String = strip_leading_list_marker(&normalize_unicode(line))
        .chars()
        .map(|c| match c {
            '\u{2010}'..='\u{2015}' | '\u{2212}' => '-',
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201C}' | '\u{201D}' => '"',
            c => c,
        })
        .collect();
    // Sites space punctuation oddly ("tuna , drained"); models tidy it.
    line.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(" ,", ",")
        .replace(" ;", ";")
        .replace(" )", ")")
        .replace("( ", "(")
}

fn tokens(text: &str) -> Vec<String> {
    normalize_line(text)
        .split(|c: char| !c.is_alphanumeric() && c != '/')
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

fn counts<'a>(items: impl IntoIterator<Item = &'a String>) -> HashMap<&'a str, usize> {
    let mut counts = HashMap::new();
    for item in items {
        *counts.entry(item.as_str()).or_insert(0) += 1;
    }
    counts
}

fn matched(want: &HashMap<&str, usize>, have: &HashMap<&str, usize>) -> usize {
    want.iter()
        .map(|(item, n)| (*n).min(have.get(item).copied().unwrap_or(0)))
        .sum()
}

/// The `lines` left over once each line in `against` has claimed one match.
fn unmatched(lines: &[String], against: &[String]) -> Vec<String> {
    let mut left = counts(against);
    lines
        .iter()
        .filter(|line| match left.get_mut(line.as_str()) {
            Some(n) if *n > 0 => {
                *n -= 1;
                false
            }
            _ => true,
        })
        .cloned()
        .collect()
}

fn ratio(n: usize, of: usize) -> f64 {
    if of == 0 {
        1.0
    } else {
        n as f64 / of as f64
    }
}

/// The numbers in a text ("2 to 2½ hours" has 2, 2 and 1/2).
fn numbers(text: &str) -> Vec<String> {
    tokens(text)
        .into_iter()
        .filter(|t| t.chars().all(|c| c.is_ascii_digit() || c == '/'))
        .collect()
}

/// One case's score.
struct Score {
    title_right: bool,
    lines_expected: usize,
    lines_answered: usize,
    missing: Vec<String>,
    extra: Vec<String>,
    instructions_recall: f64,
    instructions_precision: f64,
    invented: Vec<&'static str>,
}

/// Scores an answer against the expected recipe. `source` is all the text
/// the model was shown, for telling a stated time ("bake 11 to 12 minutes")
/// from an invented one.
fn score(answer: &RawRecipe, expected: &Expected, source: &str) -> Score {
    let answered: Vec<String> = answer
        .ingredients
        .lines()
        .map(normalize_line)
        .filter(|l| !l.is_empty())
        .collect();
    let wanted: Vec<String> = expected
        .ingredients
        .iter()
        .map(|l| normalize_line(l))
        .collect();

    let expected_tokens = tokens(&expected.instructions);
    let also_allowed = tokens(&expected.also_allowed);
    let answer_tokens = tokens(&answer.instructions);
    let notes = tokens(answer.notes.as_deref().unwrap_or_default());
    // Text moved to notes (a closing tip) isn't lost.
    let kept = counts(answer_tokens.iter().chain(&notes));
    let allowed = counts(expected_tokens.iter().chain(&also_allowed));

    let stated: HashSet<String> = numbers(source).into_iter().collect();
    let invented = [
        ("servings", &answer.servings),
        ("prep_time", &answer.prep_time),
        ("cook_time", &answer.cook_time),
        ("total_time", &answer.total_time),
        ("nutritional_info", &answer.nutritional_info),
    ]
    .into_iter()
    .filter(|(_, value)| {
        value
            .as_deref()
            .is_some_and(|v| numbers(v).iter().any(|n| !stated.contains(n)))
    })
    .map(|(name, _)| name)
    .collect();

    Score {
        title_right: normalize_line(&answer.title) == normalize_line(&expected.title),
        lines_expected: wanted.len(),
        lines_answered: answered.len(),
        missing: unmatched(&wanted, &answered),
        extra: unmatched(&answered, &wanted),
        instructions_recall: ratio(
            matched(&counts(&expected_tokens), &kept),
            expected_tokens.len(),
        ),
        instructions_precision: ratio(
            matched(&counts(&answer_tokens), &allowed),
            answer_tokens.len(),
        ),
        invented,
    }
}

fn mean(values: &[f64]) -> String {
    if values.is_empty() {
        return "–".into();
    }
    format!(
        "{:.0}%",
        100.0 * values.iter().sum::<f64>() / values.len() as f64
    )
}

/// Totals over the cases a model answered.
#[derive(Default)]
struct Totals {
    recipes: usize,
    titles: usize,
    lines_expected: usize,
    lines_missing: usize,
    lines_answered: usize,
    lines_extra: usize,
    recall: Vec<f64>,
    precision: Vec<f64>,
    invented: usize,
    misses: Vec<(f64, String)>,
}

impl Totals {
    fn add(&mut self, id: &str, score: Score) {
        self.recipes += 1;
        self.titles += usize::from(score.title_right);
        self.lines_expected += score.lines_expected;
        self.lines_missing += score.missing.len();
        self.lines_answered += score.lines_answered;
        self.lines_extra += score.extra.len();
        self.recall.push(score.instructions_recall);
        self.precision.push(score.instructions_precision);
        self.invented += score.invented.len();
        let badness = ratio(score.missing.len(), score.lines_expected.max(1))
            + 0.5 * ratio(score.extra.len(), score.lines_answered.max(1))
            + (1.0 - score.instructions_recall)
            + (1.0 - score.instructions_precision)
            + 0.25 * score.invented.len() as f64
            + if score.title_right { 0.0 } else { 0.25 };
        if badness > 0.0 {
            let mut parts = Vec::new();
            if !score.title_right {
                parts.push("wrong title".to_string());
            }
            if !score.missing.is_empty() {
                parts.push(format!("missing {:?}", first(&score.missing)));
            }
            if !score.extra.is_empty() {
                parts.push(format!("extra {:?}", first(&score.extra)));
            }
            parts.push(format!(
                "instructions recall {:.0}%, precision {:.0}%",
                100.0 * score.instructions_recall,
                100.0 * score.instructions_precision
            ));
            if !score.invented.is_empty() {
                parts.push(format!("invented {}", score.invented.join(", ")));
            }
            self.misses
                .push((badness, format!("{id}: {}", parts.join("; "))));
        }
    }

    /// The columns both suites share, before Invalid.
    fn cells(&self) -> Vec<String> {
        vec![
            pct(self.titles, self.recipes),
            pct(
                self.lines_expected - self.lines_missing,
                self.lines_expected,
            ),
            pct(self.lines_extra, self.lines_answered),
            mean(&self.recall),
            mean(&self.precision),
            self.invented.to_string(),
        ]
    }
}

fn first(lines: &[String]) -> &[String] {
    &lines[..lines.len().min(3)]
}

pub const TEXT_COLUMNS: &[&str] = &[
    "Title right",
    "Ingredient lines found",
    "Extra ingredient lines",
    "Instructions recall",
    "Instructions precision",
    "Invented fields",
    "Invalid",
    "Not a recipe: left empty",
];

/// The text columns without the non-recipe one.
pub const PHOTO_COLUMNS: &[&str] = TEXT_COLUMNS.split_at(TEXT_COLUMNS.len() - 1).0;

/// Asks every case once, as production does (no retry), and dumps the
/// answers. Returns each case's answer (None if invalid) and the invalid
/// count.
async fn extract_all<C: Serialize>(
    suite: &str,
    model: &str,
    cases: &[C],
    spend: &mut Spend,
    mut ask: impl AsyncFnMut(usize) -> Result<(RawRecipe, Usage), AiError>,
) -> Result<(Vec<Option<RawRecipe>>, usize)> {
    let indexes: Vec<usize> = (0..cases.len()).collect();
    let mut answers = vec![None; cases.len()];
    let invalid = in_batches(
        &indexes,
        1,
        Split::Singles,
        spend,
        async |batch: &[usize]| {
            let i = batch[0];
            ask(i).await.map(|(recipe, usage)| ((i, recipe), usage))
        },
        |(i, recipe)| answers[i] = Some(recipe),
    )
    .await?;
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

pub async fn eval_text(model: &str, cases: &[TextCase], spend: &mut Spend) -> Result<ModelResult> {
    let client = client_for(model, AiConfig::for_extraction)?;
    let (answers, invalid) = extract_all("text-extraction", model, cases, spend, async |i| {
        extract_recipe_from_text(&client, &cases[i].text)
            .await
            .map(|result| (result.raw_recipe, result.usage))
    })
    .await?;
    let mut totals = Totals::default();
    let mut left_empty = 0;
    for (case, answer) in cases.iter().zip(&answers) {
        let Some(answer) = answer else {
            continue;
        };
        match &case.expected {
            Some(expected) => totals.add(&case.id, score(answer, expected, &case.text)),
            None if answer.ingredients.trim().is_empty()
                && answer.instructions.trim().is_empty() =>
            {
                left_empty += 1
            }
            None => totals.misses.push((
                2.0,
                format!("{}: made a recipe of {:?}", case.id, answer.title),
            )),
        }
    }
    let not_recipes = cases.iter().filter(|c| c.expected.is_none()).count();
    let mut cells = totals.cells();
    cells.push(pct(invalid, cases.len()));
    cells.push(pct(left_empty, not_recipes));
    Ok(ModelResult {
        model: model.into(),
        cells,
        misses: worst(totals.misses),
    })
}

/// Each case's photos as photo import sends them: the file's bytes, typed by
/// their content.
pub fn load_photos(root: &Path, cases: &[PhotoCase]) -> Result<Vec<Vec<ImageData>>> {
    cases
        .iter()
        .map(|case| {
            case.photos
                .iter()
                .map(|name| {
                    let path = root.join(PHOTOS_DIR).join(name);
                    let bytes = fs::read(&path)
                        .with_context(|| format!("Failed to read {}", path.display()))?;
                    let content_type =
                        validate_image(&bytes).map_err(|e| anyhow!("{}: {e}", path.display()))?;
                    Ok(ImageData {
                        base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                        content_type,
                    })
                })
                .collect()
        })
        .collect()
}

pub async fn eval_photos(
    model: &str,
    cases: &[PhotoCase],
    images: &[Vec<ImageData>],
    spend: &mut Spend,
) -> Result<ModelResult> {
    let client = client_for(model, AiConfig::for_extraction)?;
    let (answers, invalid) = extract_all("photo-extraction", model, cases, spend, async |i| {
        extract_recipe_from_photos(&client, images[i].clone())
            .await
            .map(|result| (result.raw_recipe, result.usage))
    })
    .await?;
    let mut totals = Totals::default();
    for (case, answer) in cases.iter().zip(&answers) {
        if let Some(answer) = answer {
            totals.add(
                &case.id,
                score(answer, &case.expected, &case.expected.source()),
            );
        }
    }
    let mut cells = totals.cells();
    cells.push(pct(invalid, cases.len()));
    Ok(ModelResult {
        model: model.into(),
        cells,
        misses: worst(totals.misses),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_compare_loosely() {
        assert_eq!(normalize_line("1½ cups  Water"), "1 1/2 cups water");
        assert_eq!(
            normalize_line("• ½ cup cider vinegar"),
            "1/2 cup cider vinegar"
        );
        assert_eq!(
            normalize_line("2 to 2½ hours — covered"),
            "2 to 2 1/2 hours - covered"
        );
        assert_eq!(
            normalize_line("1 can tuna , drained"),
            "1 can tuna, drained"
        );
    }

    fn recipe(ingredients: &str, instructions: &str) -> RawRecipe {
        RawRecipe {
            title: "Red Rice".into(),
            description: None,
            ingredients: ingredients.into(),
            instructions: instructions.into(),
            image_urls: vec![],
            source_url: None,
            source_name: None,
            servings: Some("4".into()),
            prep_time: None,
            cook_time: None,
            total_time: None,
            rating: None,
            difficulty: None,
            nutritional_info: None,
            notes: None,
            categories: None,
            footnotes: None,
        }
    }

    fn expected() -> Expected {
        Expected {
            title: "RED RICE".into(),
            ingredients: vec!["1 cup rice".into(), "1 bay leaf".into()],
            instructions: "Simmer the rice.".into(),
            also_allowed: "Stir in cumin.".into(),
            servings: Some("4".into()),
            prep_time: None,
            cook_time: None,
            total_time: None,
        }
    }

    #[test]
    fn scores_lines_instructions_and_inventions() {
        let mut answer = recipe(
            "1 cup rice\n\n2 cups stock",
            "Simmer the rice. Stir in cumin. Serve hot.",
        );
        answer.cook_time = Some("20 to 25 minutes".into());
        answer.total_time = Some("45 minutes".into());
        let score = score(&answer, &expected(), "Serves 4. Simmer 20 to 25 minutes.");
        assert!(score.title_right);
        assert_eq!((score.missing.len(), score.lines_expected), (1, 2));
        assert_eq!(score.missing, vec!["1 bay leaf"]);
        assert_eq!(score.extra, vec!["2 cups stock"]);
        assert_eq!(score.instructions_recall, 1.0);
        // "serve" and "hot" are neither expected nor allowed.
        assert_eq!(score.instructions_precision, 6.0 / 8.0);
        // The cook time is stated; the total isn't.
        assert_eq!(score.invented, vec!["total_time"]);
    }
}
