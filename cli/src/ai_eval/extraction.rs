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
    /// The page has prose not transcribed here (a long headnote), so an
    /// answer's description and notes can't be checked against the source.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    prose_untranscribed: bool,
    /// What the source states for these. An answer must keep their numbers
    /// (a stated time in any of its time fields, since pages label times
    /// loosely) and invent none.
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
                    prose_untranscribed: false,
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

fn is_number(token: &str) -> bool {
    token.chars().all(|c| c.is_ascii_digit() || c == '/')
}

/// The numbers in a text ("2 to 2½ hours" has 2, 2 and 1/2).
fn numbers(text: &str) -> Vec<String> {
    tokens(text).into_iter().filter(|t| is_number(t)).collect()
}

/// What a number in a recipe counts.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Quantity {
    Servings,
    Time,
    Nutrition,
}

/// The quantity a word says its numbers count ("serves 4", "30 minutes",
/// "410 calories").
fn cue(word: &str) -> Option<Quantity> {
    if matches!(
        word,
        "serves" | "serving" | "servings" | "makes" | "yield" | "yields" | "people" | "portions"
    ) {
        Some(Quantity::Servings)
    } else if ["minute", "hour", "second"]
        .iter()
        .any(|p| word.starts_with(p))
        || matches!(word, "min" | "mins" | "hr" | "hrs")
    {
        Some(Quantity::Time)
    } else if [
        "cal", "kcal", "fat", "protein", "carb", "sodium", "sugar", "fiber",
    ]
    .iter()
    .any(|p| word.starts_with(p))
    {
        Some(Quantity::Nutrition)
    } else {
        None
    }
}

/// Each number a text states, with what it counts. A run of numbers ("5 to
/// 6", "1 1/2") counts the time or nutrition word right after it ("5 to 6
/// minutes", "410 calories", or "cal 410" before it). Otherwise it counts
/// servings if a servings word came earlier in its sentence ("serves 4
/// generously, 6 moderately"), and nothing if not ("130 to 135 degrees").
fn quantities(text: &str) -> HashSet<(String, Quantity)> {
    let mut stated = HashSet::new();
    for sentence in text.split(['.', ';', '\n']) {
        let words = tokens(sentence);
        let mut after_servings_word = false;
        let mut i = 0;
        while i < words.len() {
            if !is_number(&words[i]) {
                // "serving" and "people" also name things ("a serving bowl"),
                // so only these reach past the next number.
                after_servings_word |= matches!(
                    words[i].as_str(),
                    "serves" | "servings" | "makes" | "yield" | "yields"
                );
                i += 1;
                continue;
            }
            let start = i;
            loop {
                if i < words.len() && is_number(&words[i]) {
                    i += 1;
                } else if i + 1 < words.len()
                    && matches!(words[i].as_str(), "to" | "or" | "and")
                    && is_number(&words[i + 1])
                {
                    i += 2;
                } else {
                    break;
                }
            }
            let before = start.checked_sub(1).and_then(|b| cue(&words[b]));
            let quantity = match (before, words.get(i).and_then(|w| cue(w))) {
                (_, Some(q @ (Quantity::Time | Quantity::Nutrition))) => Some(q),
                (Some(Quantity::Nutrition), _) => Some(Quantity::Nutrition),
                (_, Some(Quantity::Servings)) => Some(Quantity::Servings),
                _ if after_servings_word => Some(Quantity::Servings),
                _ => None,
            };
            if let Some(quantity) = quantity {
                for word in words[start..i].iter().filter(|w| is_number(w)) {
                    stated.insert((word.clone(), quantity));
                }
            }
        }
    }
    stated
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
    /// Servings and time fields the source states, and those the answer kept.
    metadata_expected: usize,
    metadata_kept: usize,
    /// The share of the page's other text kept, if it has any.
    extra_text_kept: Option<f64>,
    /// Words of the description and notes, and those the source never uses.
    note_words: usize,
    unsourced_note_words: usize,
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

    let stated = quantities(source);
    let source_words: HashSet<String> = tokens(source).into_iter().collect();
    let unsourced = |text: &str| tokens(text).iter().any(|w| !source_words.contains(w));
    let mut invented: Vec<&'static str> = [
        ("servings", &answer.servings, Quantity::Servings),
        ("prep_time", &answer.prep_time, Quantity::Time),
        ("cook_time", &answer.cook_time, Quantity::Time),
        ("total_time", &answer.total_time, Quantity::Time),
        (
            "nutritional_info",
            &answer.nutritional_info,
            Quantity::Nutrition,
        ),
    ]
    .into_iter()
    .filter(|(_, value, quantity)| {
        value.as_deref().is_some_and(|v| {
            let numbers = numbers(v);
            if numbers.is_empty() {
                // "serves a crowd", "quick": words the source never uses.
                unsourced(v)
            } else {
                numbers
                    .into_iter()
                    .any(|n| !stated.contains(&(n, *quantity)))
            }
        })
    })
    .map(|(name, _, _)| name)
    .collect();
    // The rest of the draft a user sees: tags come from categories.
    for (name, value) in [
        ("difficulty", &answer.difficulty),
        ("source_name", &answer.source_name),
        ("source_url", &answer.source_url),
    ] {
        if value.as_deref().is_some_and(unsourced) {
            invented.push(name);
        }
    }
    if answer
        .categories
        .iter()
        .flatten()
        .any(|category| unsourced(category))
    {
        invented.push("categories");
    }
    if answer
        .rating
        .is_some_and(|rating| !source_words.contains(&rating.to_string()))
    {
        invented.push("rating");
    }

    // Every number, as often as it appears: "2 hours, plus 2 hours
    // chilling" isn't kept by "2 hours".
    let has_numbers = |value: &Option<String>, of: &str| {
        let have = numbers(value.as_deref().unwrap_or_default());
        let want = numbers(of);
        matched(&counts(&want), &counts(&have)) == want.len()
    };
    let answered_times = [&answer.prep_time, &answer.cook_time, &answer.total_time];
    let stated_metadata: Vec<bool> = [
        expected
            .servings
            .as_ref()
            .map(|servings| has_numbers(&answer.servings, servings)),
        expected
            .prep_time
            .as_ref()
            .map(|t| answered_times.iter().any(|a| has_numbers(a, t))),
        expected
            .cook_time
            .as_ref()
            .map(|t| answered_times.iter().any(|a| has_numbers(a, t))),
        expected
            .total_time
            .as_ref()
            .map(|t| answered_times.iter().any(|a| has_numbers(a, t))),
    ]
    .into_iter()
    .flatten()
    .collect();

    // Notes and the description are free text, so they're checked only for
    // words the source never uses.
    // The page's other text (a description, headnote or variation) should
    // survive somewhere: the description, notes or instructions.
    let extra_text = tokens(&expected.also_allowed);
    let description = tokens(answer.description.as_deref().unwrap_or_default());
    let extra_text_kept = (!extra_text.is_empty()).then(|| {
        let answered = counts(answer_tokens.iter().chain(&notes).chain(&description));
        ratio(matched(&counts(&extra_text), &answered), extra_text.len())
    });
    let note_words: Vec<String> = if expected.prose_untranscribed {
        Vec::new()
    } else {
        [&answer.description, &answer.notes]
            .into_iter()
            .flatten()
            .flat_map(|text| tokens(text))
            .collect()
    };

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
        extra_text_kept,
        metadata_expected: stated_metadata.len(),
        metadata_kept: stated_metadata.iter().filter(|kept| **kept).count(),
        unsourced_note_words: note_words
            .iter()
            .filter(|w| !source_words.contains(*w))
            .count(),
        note_words: note_words.len(),
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
    extra_text: Vec<f64>,
    invented: usize,
    metadata_expected: usize,
    metadata_kept: usize,
    note_words: usize,
    unsourced_note_words: usize,
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
        self.extra_text.extend(score.extra_text_kept);
        self.precision.push(score.instructions_precision);
        self.invented += score.invented.len();
        self.metadata_expected += score.metadata_expected;
        self.metadata_kept += score.metadata_kept;
        self.note_words += score.note_words;
        self.unsourced_note_words += score.unsourced_note_words;
        let badness = ratio(score.missing.len(), score.lines_expected.max(1))
            + 0.5 * ratio(score.extra.len(), score.lines_answered.max(1))
            + (1.0 - score.instructions_recall)
            + (1.0 - score.instructions_precision)
            + (1.0 - score.extra_text_kept.unwrap_or(1.0))
            + 0.25 * score.invented.len() as f64
            + 0.25 * (score.metadata_expected - score.metadata_kept) as f64
            + ratio(score.unsourced_note_words, score.note_words.max(1))
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
            if let Some(kept) = score.extra_text_kept.filter(|k| *k < 1.0) {
                parts.push(format!("kept {:.0}% of the other text", 100.0 * kept));
            }
            if score.metadata_kept < score.metadata_expected {
                parts.push(format!(
                    "kept {} of {} stated servings and times",
                    score.metadata_kept, score.metadata_expected
                ));
            }
            if score.unsourced_note_words > 0 {
                parts.push(format!(
                    "{} of {} note/description words not in the source",
                    score.unsourced_note_words, score.note_words
                ));
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
            mean(&self.extra_text),
            pct(self.metadata_kept, self.metadata_expected),
            self.invented.to_string(),
            pct(self.unsourced_note_words, self.note_words),
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
    "Other text kept",
    "Servings and times kept",
    "Invented fields",
    "Unsourced note words",
    "Invalid",
    "Not a recipe: left empty",
    "Recipes with warnings",
];

/// The text columns without the two that only text extraction has.
pub const PHOTO_COLUMNS: &[&str] = TEXT_COLUMNS.split_at(TEXT_COLUMNS.len() - 2).0;

/// An extraction: the recipe, and the warnings text import shows (and that
/// make it skip enrichment). Photo import has none.
#[derive(Clone, Serialize)]
struct Answer {
    recipe: RawRecipe,
    warnings: Vec<String>,
}

/// Asks every case once, as production does (no retry), and dumps the
/// answers. Returns each case's answer (None if invalid) and the invalid
/// count.
async fn extract_all<C: Serialize>(
    suite: &str,
    model: &str,
    cases: &[C],
    spend: &mut Spend,
    mut ask: impl AsyncFnMut(usize) -> Result<(Answer, Usage), AiError>,
) -> Result<(Vec<Option<Answer>>, usize)> {
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

/// Whether an answer to a text that isn't a recipe left every field a user
/// would see empty, as the prompt asks.
fn left_blank(answer: &RawRecipe) -> bool {
    [&answer.title, &answer.ingredients, &answer.instructions]
        .into_iter()
        .all(|field| field.trim().is_empty())
        && [
            &answer.description,
            &answer.servings,
            &answer.prep_time,
            &answer.cook_time,
            &answer.total_time,
            &answer.nutritional_info,
            &answer.notes,
            &answer.difficulty,
        ]
        .into_iter()
        .all(|field| field.as_deref().is_none_or(|v| v.trim().is_empty()))
        && [&answer.source_name, &answer.source_url]
            .into_iter()
            .all(Option::is_none)
        && answer.categories.as_ref().is_none_or(Vec::is_empty)
        && answer.rating.is_none()
}

pub async fn eval_text(model: &str, cases: &[TextCase], spend: &mut Spend) -> Result<ModelResult> {
    let client = client_for(model, AiConfig::for_extraction)?;
    let (answers, invalid) = extract_all("text-extraction", model, cases, spend, async |i| {
        extract_recipe_from_text(&client, &cases[i].text)
            .await
            .map(|result| {
                let answer = Answer {
                    recipe: result.raw_recipe,
                    warnings: result.warnings,
                };
                (answer, result.usage)
            })
    })
    .await?;
    let mut totals = Totals::default();
    let (mut left_empty, mut recipes, mut warned) = (0, 0, 0);
    for (case, answer) in cases.iter().zip(&answers) {
        let Some(Answer {
            recipe: answer,
            warnings,
        }) = answer
        else {
            continue;
        };
        match &case.expected {
            Some(expected) => {
                totals.add(&case.id, score(answer, expected, &case.text));
                recipes += 1;
                // Text import skips title, description and tag enrichment
                // when there's any warning.
                if let Some(warning) = warnings.first() {
                    warned += 1;
                    totals
                        .misses
                        .push((0.5, format!("{}: warned {warning:?}", case.id)));
                }
            }
            None if left_blank(answer) => left_empty += 1,
            None => totals.misses.push((
                2.0,
                format!(
                    "{}: filled in fields of a non-recipe (title {:?})",
                    case.id, answer.title
                ),
            )),
        }
    }
    let not_recipes = cases.iter().filter(|c| c.expected.is_none()).count();
    let mut cells = totals.cells();
    cells.push(pct(invalid, cases.len()));
    cells.push(pct(left_empty, not_recipes));
    cells.push(pct(warned, recipes));
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
            .map(|result| {
                let answer = Answer {
                    recipe: result.raw_recipe,
                    warnings: Vec::new(),
                };
                (answer, result.usage)
            })
    })
    .await?;
    let mut totals = Totals::default();
    for (case, answer) in cases.iter().zip(&answers) {
        if let Some(answer) = answer {
            totals.add(
                &case.id,
                score(&answer.recipe, &case.expected, &case.expected.source()),
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
            prose_untranscribed: false,
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
        answer.notes = Some("Simmer gently. Garnish with chives.".into());
        let score = score(&answer, &expected(), "Serves 4. Simmer 20 to 25 minutes.");
        assert!(score.title_right);
        assert_eq!((score.missing.len(), score.lines_expected), (1, 2));
        assert_eq!(score.missing, vec!["1 bay leaf"]);
        assert_eq!(score.extra, vec!["2 cups stock"]);
        assert_eq!(score.instructions_recall, 1.0);
        // "serve" and "hot" are neither expected nor allowed.
        assert_eq!(score.instructions_precision, 6.0 / 8.0);
        // The page's other text ("Stir in cumin") is kept in the instructions.
        assert_eq!(score.extra_text_kept, Some(1.0));
        // The cook time is stated; the total isn't.
        assert_eq!(score.invented, vec!["total_time"]);
        let mut tagged = recipe("1 cup rice", "Simmer the rice.");
        tagged.categories = Some(vec!["rice".into(), "weeknight".into()]);
        tagged.rating = Some(5);
        let tagged = super::score(&tagged, &expected(), "Serves 4. Simmer the rice.");
        assert_eq!(tagged.invented, vec!["categories", "rating"]);
        // The stated servings are kept.
        assert_eq!((score.metadata_kept, score.metadata_expected), (1, 1));
        // Only "simmer" is in the source.
        assert_eq!((score.unsourced_note_words, score.note_words), (4, 5));
    }

    #[test]
    fn a_kept_time_needs_each_of_its_numbers() {
        let mut answer = recipe("1 cup rice\n1 bay leaf", "Simmer the rice.");
        answer.total_time = Some("2 hours".into());
        let expected = Expected {
            total_time: Some("2 hours, plus 2 hours chilling".into()),
            ..expected()
        };
        let score = score(
            &answer,
            &expected,
            "Serves 4. 2 hours, plus 2 hours chilling.",
        );
        // The servings are kept; the chilling time isn't.
        assert_eq!((score.metadata_kept, score.metadata_expected), (1, 2));
    }

    #[test]
    fn numbers_count_what_their_nearest_cue_says() {
        let stated = quantities(
            "Serves 4. Bake 30 minutes, then rest 11 to 12 minutes.\nyield: approximately 18 to 24 cookies\nCook until it registers 130 to 135 degrees, 5 to 6 minutes per side.\nPer serving: Cal 410",
        );
        assert!(stated.contains(&("6".into(), Quantity::Time)));
        assert!(!stated.contains(&("130".into(), Quantity::Time)));
        assert!(stated.contains(&("410".into(), Quantity::Nutrition)));
        let prose = quantities("Transfer to a serving bowl and garnish with about 1/3 cup chives.");
        assert!(!prose.contains(&("1/3".into(), Quantity::Servings)));
        assert!(stated.contains(&("24".into(), Quantity::Servings)));
        assert!(stated.contains(&("4".into(), Quantity::Servings)));
        assert!(stated.contains(&("30".into(), Quantity::Time)));
        assert!(stated.contains(&("11".into(), Quantity::Time)));
        assert!(!stated.contains(&("30".into(), Quantity::Servings)));
        assert!(!stated.contains(&("4".into(), Quantity::Time)));
    }
}
