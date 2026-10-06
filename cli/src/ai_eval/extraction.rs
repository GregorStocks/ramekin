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
    SNAPSHOTS_DIR,
};

pub const PHOTOS_DIR: &str = "data/ai-evals/photos";

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
    /// Source text that may appear in an answer's description or notes but
    /// needn't: a personal aside around a pasted recipe (unlike the
    /// boilerplate beside it).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    incidental: String,
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
    /// The nutrition the source prints (a per-serving panel), which an
    /// answer must keep in its nutrition field with the same numbers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    nutritional_info: Option<String>,
}

impl Expected {
    /// Everything a photo case's page says, as text: what its fields
    /// transcribe.
    fn source(&self) -> String {
        let mut parts = vec![
            self.title.clone(),
            self.instructions.clone(),
            self.also_allowed.clone(),
        ];
        parts.extend(self.ingredients.iter().cloned());
        parts.extend(self.servings.clone());
        parts.extend(self.nutritional_info.clone());
        // Labelled as the page labels them, so they count as stated times.
        for (label, time) in [
            ("Prep time", &self.prep_time),
            ("Cook time", &self.cook_time),
            ("Total time", &self.total_time),
        ] {
            parts.extend(time.as_ref().map(|time| format!("{label}: {time}")));
        }
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

/// Blog or personal text around a pasted recipe: before it, after it, and
/// the personal asides in it that a description or note may keep (the rest
/// is boilerplate that shouldn't reach the draft).
const CHATTER: [(&str, &str, &str); 2] = [
    (
        "Posted by Jen · March 3 · 14 comments\n\nThis has been our family's favorite for years! The kids ask for it every single week, and it freezes beautifully. Scroll down for the printable recipe card.\n\nJump to Recipe · Print Recipe\n",
        "\nDid you make this? Leave a star rating and tag us on Instagram!\n\nYou might also like: Easy Weeknight Chili · The Best Banana Bread · 5-Minute Guacamole\n\n© 2025 Jen's Kitchen. All rights reserved.",
        "This has been our family's favorite for years! The kids ask for it every single week, and it freezes beautifully.",
    ),
    (
        "Mom's recipe, copied from the card in her recipe box. She always made this for Sunday dinner.\n\n",
        "\nNote to self: double it next time, it went fast.",
        "Mom's recipe, copied from the card in her recipe box. She always made this for Sunday dinner. Note to self: double it next time, it went fast.",
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
            if let Some((before, _, _)) = chatter {
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
            if let Some((_, after, _)) = chatter {
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
                    incidental: chatter.map_or("", |(_, _, aside)| aside).into(),
                    prose_untranscribed: false,
                    servings,
                    prep_time: None,
                    cook_time: None,
                    total_time: None,
                    nutritional_info: None,
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

/// Whether the character at `i` is a decimal point ("12.5") or fraction
/// slash ("1/2"), which stays in its number rather than ending a word or
/// sentence.
fn decimal_point(chars: &[char], i: usize) -> bool {
    matches!(chars[i], '.' | '/')
        && i > 0
        && chars[i - 1].is_ascii_digit()
        && chars.get(i + 1).is_some_and(char::is_ascii_digit)
}

fn tokens(text: &str) -> Vec<String> {
    let chars: Vec<char> = normalize_line(text).chars().collect();
    let mut words = Vec::new();
    let mut word = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_alphabetic() && !word.is_empty() && is_number(&word) {
            // "227g" is 227 g: the number on its own, as in "227 g".
            words.push(std::mem::take(&mut word));
            word.push(c);
        } else if c.is_alphanumeric() || decimal_point(&chars, i) {
            word.push(c);
        } else if !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// A text's sentences: split at periods (not decimal points), semicolons
/// and line breaks.
fn sentences(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut sentences = vec![String::new()];
    for (i, &c) in chars.iter().enumerate() {
        if matches!(c, ';' | '\n') || (c == '.' && !decimal_point(&chars, i)) {
            sentences.push(String::new());
        } else if let Some(sentence) = sentences.last_mut() {
            sentence.push(c);
        }
    }
    sentences
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
    token
        .chars()
        .all(|c| c.is_ascii_digit() || c == '/' || c == '.')
}

/// What a number in a recipe counts.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Quantity {
    Servings,
    Seconds,
    Minutes,
    Hours,
    Nutrition,
}

impl Quantity {
    fn is_time(self) -> bool {
        matches!(self, Self::Seconds | Self::Minutes | Self::Hours)
    }
}

/// What a nutrition panel's words start with, one per nutrient. Calcium
/// comes before calories, which "Cal" would otherwise also match.
const NUTRIENTS: [&str; 9] = [
    "calcium", "cal", "fat", "chol", "protein", "carb", "sodium", "sugar", "fiber",
];

/// The nutrient a word names, as its `NUTRIENTS` prefix: "Cal",
/// "Calories" and "kcal" all name calories.
fn nutrient(word: &str) -> Option<&'static str> {
    let word = if word == "kcal" { "cal" } else { word };
    NUTRIENTS.iter().find(|p| word.starts_with(**p)).copied()
}

/// Each nutrient a text gives a number for, as "nutrient number" ("fat 10",
/// "sat fat 4"). Within each item of the panel (split at "•", commas,
/// semicolons and line breaks), a number's nutrient is the word before it
/// ("Total Fat 10g") or else the first word after it but a unit ("410
/// calories", "7 g protein"). Saturated fat and added sugar keep their
/// qualifier, so they aren't confused with the totals. Numbers with no
/// nutrient ("Per serving (6)") are left out.
fn nutrients(text: &str) -> Vec<String> {
    let mut stated = Vec::new();
    for item in text.split(['•', '·', '|', ',', ';', '\n']) {
        let words = tokens(item);
        for (i, word) in words.iter().enumerate() {
            if !is_number(word) {
                continue;
            }
            let after = words[i + 1..]
                .iter()
                .position(|w| !matches!(w.as_str(), "g" | "mg" | "mcg" | "grams" | "milligrams"))
                .map(|skip| i + 1 + skip);
            let at = if i > 0 && nutrient(&words[i - 1]).is_some() {
                i - 1
            } else if let Some(at) = after.filter(|&at| {
                nutrient(&words[at]).is_some()
                    // In "(6) Cal 780", "Cal" is 780's label.
                    && !words.get(at + 1).is_some_and(|w| is_number(w))
            }) {
                at
            } else {
                continue;
            };
            let label = nutrient(&words[at]).unwrap_or_default();
            let qualifier = at.checked_sub(1).and_then(|q| {
                ["sat", "add", "trans"]
                    .into_iter()
                    .find(|p| words[q].starts_with(p))
            });
            stated.push(match qualifier {
                Some(q) => format!("{q} {label} {word}"),
                None => format!("{label} {word}"),
            });
        }
    }
    stated
}

/// The quantity a word says its numbers count ("serves 4", "30 minutes",
/// "410 calories").
fn cue(word: &str) -> Option<Quantity> {
    if matches!(
        word,
        "serves" | "serving" | "servings" | "makes" | "yield" | "yields" | "people" | "portions"
    ) {
        Some(Quantity::Servings)
    } else if word.starts_with("second") {
        Some(Quantity::Seconds)
    } else if word.starts_with("minute") || matches!(word, "min" | "mins") {
        Some(Quantity::Minutes)
    } else if word.starts_with("hour") || matches!(word, "hr" | "hrs") {
        Some(Quantity::Hours)
    } else if nutrient(word).is_some() {
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
/// Whether a sentence labels a recipe time ("Prep time: 10 minutes", "Cook:
/// 20 min", "TOTAL TIME", "Ready in 30 minutes"), as opposed to a step that
/// happens to say "cook ... about 2 minutes".
fn labels_a_time(sentence: &str) -> bool {
    let words: Vec<&str> = sentence.split_whitespace().collect();
    let text = words.join(" ").to_lowercase();
    ["prep", "cook", "cooking", "total", "active", "time"]
        .iter()
        .any(|label| text.contains(&format!("{label}:")) || text.contains(&format!("{label} :")))
        || [
            "prep time",
            "cook time",
            "cooking time",
            "total time",
            "active time",
            "ready in",
        ]
        .iter()
        .any(|phrase| text.contains(phrase))
}

/// The servings, times and nutrition a text states as a recipe's metadata,
/// counted so a quantity stated once backs one answered field. A time counts
/// only in a sentence that labels one ("Prep: 10 minutes", "Total time",
/// "ready in"): a step's "chill 30 minutes" isn't the recipe's total time.
fn quantities(text: &str) -> HashMap<(String, Quantity), usize> {
    let mut stated = HashMap::new();
    for sentence in sentences(text) {
        let labelled = labels_a_time(&sentence);
        for (number, quantity) in classify(&sentence) {
            if let Some(quantity) = quantity.filter(|q| labelled || !q.is_time()) {
                *stated.entry((number, quantity)).or_insert(0) += 1;
            }
        }
    }
    stated
}

/// Every number in a text, in order, with what it counts if anything says
/// (see `quantities`).
fn classify(text: &str) -> Vec<(String, Option<Quantity>)> {
    let mut stated = Vec::new();
    for sentence in sentences(text) {
        let words = tokens(&sentence);
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
                (_, Some(q)) if q != Quantity::Servings => Some(q),
                (Some(Quantity::Nutrition), _) => Some(Quantity::Nutrition),
                (_, Some(Quantity::Servings)) => Some(Quantity::Servings),
                _ if after_servings_word => Some(Quantity::Servings),
                _ => None,
            };
            for word in words[start..i].iter().filter(|w| is_number(w)) {
                stated.push((word.clone(), quantity));
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
    /// Servings, time and nutrition fields the source states, and those the
    /// answer kept.
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

    // Each quantity the source states backs one answered value: "Bake 30
    // minutes" doesn't make prep, cook and total time all 30 minutes.
    let mut stated = quantities(source);
    let mut take = |number: String, quantity: Option<Quantity>| {
        // A bare number in a time field may be in any unit the source uses.
        let units = match quantity {
            Some(quantity) => vec![quantity],
            None => vec![Quantity::Seconds, Quantity::Minutes, Quantity::Hours],
        };
        units
            .into_iter()
            .any(|unit| match stated.get_mut(&(number.clone(), unit)) {
                Some(left) if *left > 0 => {
                    *left -= 1;
                    true
                }
                _ => false,
            })
    };
    let source_words: HashSet<String> = tokens(source).into_iter().collect();
    let unsourced = |text: &str| tokens(text).iter().any(|w| !source_words.contains(w));
    let mut invented: Vec<&'static str> = Vec::new();
    for (name, value, quantity) in [
        ("servings", &answer.servings, Some(Quantity::Servings)),
        ("prep_time", &answer.prep_time, None),
        ("cook_time", &answer.cook_time, None),
        ("total_time", &answer.total_time, None),
        (
            "nutritional_info",
            &answer.nutritional_info,
            Some(Quantity::Nutrition),
        ),
    ] {
        let Some(value) = value.as_deref() else {
            continue;
        };
        // Everything but the numbers and their labels must be the source's:
        // "serves a crowd", "quick", "Total Fat 999g".
        let unsourced_rest = tokens(value).iter().any(|w| {
            !is_number(w)
                && cue(w).is_none()
                // Connectors and field labels ("TOTAL TIME: 2 hours").
                && !matches!(
                    w.as_str(),
                    "to" | "or" | "and" | "plus" | "about" | "approximately"
                        | "total" | "time" | "prep" | "cook" | "active"
                )
                && !source_words.contains(w)
        });
        // "30 hours" isn't the source's "30 minutes".
        let mut unstated = false;
        for (number, said) in classify(value) {
            unstated |= !take(number, said.or(quantity));
        }
        if unsourced_rest || unstated {
            invented.push(name);
        }
    }
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
    // A rating needs the source to rate ("4 stars", "rated 5"), not just to
    // contain the digit somewhere.
    let source_tokens = tokens(source);
    let rated = |rating: i32| {
        let rating = rating.to_string();
        source_tokens.iter().enumerate().any(|(i, word)| {
            *word == rating
                && source_tokens[i.saturating_sub(2)..(i + 3).min(source_tokens.len())]
                    .iter()
                    .any(|w| w.starts_with("star") || w.starts_with("rat"))
        })
    };
    if answer.rating.is_some_and(|rating| !rated(rating)) {
        invented.push("rating");
    }

    // Every number with its unit, as often as it appears: "2 hours, plus 2
    // hours chilling" isn't kept by "2 hours", nor "30 minutes" by "30 hours".
    let has_numbers = |value: &Option<String>, of: &str| {
        let keyed = |text: &str| -> Vec<String> {
            classify(text)
                .into_iter()
                .map(|(n, q)| format!("{n} {:?}", q.filter(|q| q.is_time())))
                .collect()
        };
        let have = keyed(value.as_deref().unwrap_or_default());
        let want = keyed(of);
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
        // Each number with its nutrient ("Cal 10 • Total Fat 410g" doesn't
        // keep "Cal 410 • Total Fat 10g"), in its field: a panel copied into
        // the notes isn't nutrition the recipe has.
        expected.nutritional_info.as_ref().map(|nutrition| {
            let want = nutrients(nutrition);
            let have = nutrients(answer.nutritional_info.as_deref().unwrap_or_default());
            matched(&counts(&want), &counts(&have)) == want.len()
        }),
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
    // Against the recipe's own text, not the whole input: blog comments and
    // a copyright line copied into notes still pollute the draft.
    let recipe_words: HashSet<String> = tokens(&expected.source())
        .into_iter()
        .chain(tokens(&expected.incidental))
        .collect();
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
        instructions_recall: {
            let in_field = ratio(
                matched(&counts(&expected_tokens), &counts(&answer_tokens)),
                expected_tokens.len(),
            );
            // Notes can hold a moved closing tip, not stand in for the steps.
            if in_field >= 0.5 {
                ratio(
                    matched(&counts(&expected_tokens), &kept),
                    expected_tokens.len(),
                )
            } else {
                in_field
            }
        },
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
            .filter(|w| !recipe_words.contains(*w))
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
                    "kept {} of {} stated servings, times and nutrition",
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
    "Servings, times and nutrition kept",
    "Invented fields",
    "Unsourced note words",
    "Invalid",
    "Not a recipe: left empty",
    "Recipes with warnings",
];

/// The text columns without the two that only text extraction has.
pub const PHOTO_COLUMNS: &[&str] = TEXT_COLUMNS.split_at(TEXT_COLUMNS.len() - 2).0;

/// An extraction: the recipe, and the warnings text import shows. Photo
/// import has none.
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
                // Text import shows warnings to the user, so one on a sound
                // recipe is noise.
                if let Some(warning) = warnings.first() {
                    warned += 1;
                    totals
                        .misses
                        .push((0.5, format!("{}: warned {warning:?}", case.id)));
                }
            }
            // The prompt asks for empty fields and a warning saying why.
            None if left_blank(answer) && !warnings.is_empty() => left_empty += 1,
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
            incidental: String::new(),
            prose_untranscribed: false,
            servings: Some("4".into()),
            prep_time: None,
            cook_time: None,
            total_time: None,
            nutritional_info: None,
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
        let score = score(
            &answer,
            &expected(),
            "Serves 4. Cook time: 20 to 25 minutes.",
        );
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
        // The source has a 5, but no rating.
        tagged.rating = Some(5);
        tagged.nutritional_info = Some("Cal 410 • Total Fat 999g".into());
        let tagged = super::score(
            &tagged,
            &expected(),
            "Serves 4. Simmer the rice 5 minutes. Cal 410.",
        );
        assert_eq!(
            tagged.invented,
            vec!["nutritional_info", "categories", "rating"]
        );
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
    fn a_printed_panel_is_kept_only_in_its_field() {
        let panel = "Per serving: Cal 410 • Total Fat 10g • Protein 7g";
        let expected = Expected {
            nutritional_info: Some(panel.into()),
            ..expected()
        };
        let source = format!("Serves 4. {panel}");
        let mut answer = recipe("1 cup rice\n1 bay leaf", "Simmer the rice.");
        answer.servings = Some("Serves 4".into());
        answer.nutritional_info = Some("Cal 410, Total Fat 10g, Protein 7g".into());
        let kept = score(&answer, &expected, &source);
        assert_eq!((kept.metadata_kept, kept.metadata_expected), (2, 2));
        assert!(kept.invented.is_empty());

        answer.nutritional_info = None;
        answer.notes = Some(panel.into());
        let in_notes = score(&answer, &expected, &source);
        assert_eq!((in_notes.metadata_kept, in_notes.metadata_expected), (1, 2));

        answer.notes = None;
        answer.nutritional_info = Some("Cal 410 • Total Fat 18g • Protein 7g".into());
        let misread = score(&answer, &expected, &source);
        assert_eq!((misread.metadata_kept, misread.metadata_expected), (1, 2));
        assert_eq!(misread.invented, vec!["nutritional_info"]);
    }

    #[test]
    fn nutrients_pair_each_number_with_its_label() {
        assert_eq!(
            nutrients("PER SERVING (6): CAL 780 • TOTAL FAT 36g • SAT FAT 11g"),
            vec!["cal 780", "fat 36", "sat fat 11"]
        );
        assert_eq!(
            nutrients("410 calories, 7 g protein, added sugars 28g"),
            vec!["cal 410", "protein 7", "add sugar 28"]
        );
        assert_eq!(
            nutrients("410 kcal • Calcium 120mg"),
            vec!["cal 410", "calcium 120"]
        );
    }

    #[test]
    fn swapped_nutrition_values_arent_kept() {
        let expected = Expected {
            nutritional_info: Some("Cal 410 • Total Fat 10g • Sat Fat 4g".into()),
            ..expected()
        };
        let mut answer = recipe("1 cup rice\n1 bay leaf", "Simmer the rice.");
        answer.nutritional_info = Some("Cal 10 • Total Fat 410g • Sat Fat 4g".into());
        let score = score(&answer, &expected, &expected.source());
        assert_eq!((score.metadata_kept, score.metadata_expected), (1, 2));
    }

    #[test]
    fn a_full_panel_copied_as_printed_is_stated() {
        let panel = "Per serving: Cal 410 • Total Fat 10g • Sat Fat 4g • Chol 55mg • \
            Sodium 380mg • Total Carbs 73g • Dietary Fiber 0g • Total Sugar 41g • \
            Added Sugar 28g • Protein 7g";
        let expected = Expected {
            servings: Some("Serves 4".into()),
            nutritional_info: Some(panel.into()),
            ..expected()
        };
        let mut answer = recipe("1 cup rice\n1 bay leaf", "Simmer the rice.");
        answer.nutritional_info = Some(panel.to_uppercase());
        let score = score(&answer, &expected, &expected.source());
        assert_eq!((score.metadata_kept, score.metadata_expected), (2, 2));
        assert!(score.invented.is_empty(), "{:?}", score.invented);
    }

    #[test]
    fn steps_in_notes_alone_dont_count() {
        let mut answer = recipe("1 cup rice\n1 bay leaf", "");
        answer.notes = Some("Simmer the rice.".into());
        let score = score(&answer, &expected(), "Serves 4. Simmer the rice.");
        assert_eq!(score.instructions_recall, 0.0);
    }

    #[test]
    fn one_stated_time_backs_one_field() {
        let mut answer = recipe("1 cup rice\n1 bay leaf", "Bake 30 minutes.");
        answer.prep_time = Some("30 minutes".into());
        answer.cook_time = Some("30 minutes".into());
        let score = score(&answer, &expected(), "Serves 4. Prep time: 30 minutes.");
        assert_eq!(score.invented, vec!["cook_time"]);
    }

    /// Every number a text gives a quantity, labelled or not.
    fn typed(text: &str) -> HashSet<(String, Quantity)> {
        classify(text)
            .into_iter()
            .filter_map(|(number, quantity)| Some((number, quantity?)))
            .collect()
    }

    #[test]
    fn only_labelled_times_are_metadata() {
        assert!(!labels_a_time("Cook, stirring, about 2 minutes"));
        assert!(labels_a_time("SERVES 4 TIME: 30 MINUTES"));
        assert!(labels_a_time("Ready in 1 hour"));
        let stated = quantities("Chill 30 minutes. Total time: 45 minutes. Serves 4.");
        assert!(!stated.contains_key(&("30".into(), Quantity::Minutes)));
        assert!(stated.contains_key(&("45".into(), Quantity::Minutes)));
        assert!(stated.contains_key(&("4".into(), Quantity::Servings)));
    }

    #[test]
    fn attached_units_split_from_their_numbers() {
        assert_eq!(
            tokens("1/2 cup, 8 oz/227g butter"),
            ["1/2", "cup", "8", "oz", "227", "g", "butter"]
        );
        assert!(typed("Total Fat 36g").contains(&("36".into(), Quantity::Nutrition)));
        assert!(!typed("8 oz (227g) butter").contains(&("227".into(), Quantity::Nutrition)));
    }

    #[test]
    fn numbers_count_what_their_nearest_cue_says() {
        let stated = typed(
            "Serves 4. Bake 30 minutes, then rest 11 to 12 minutes.\nyield: approximately 18 to 24 cookies\nCook until it registers 130 to 135 degrees, 5 to 6 minutes per side.\nPer serving: Cal 410",
        );
        assert!(stated.contains(&("6".into(), Quantity::Minutes)));
        assert!(!stated.contains(&("130".into(), Quantity::Minutes)));
        assert!(stated.contains(&("410".into(), Quantity::Nutrition)));
        assert!(!stated.contains(&("30".into(), Quantity::Hours)));
        let decimal = typed("Cook 12.5 minutes. Serves 4.");
        assert!(decimal.contains(&("12.5".into(), Quantity::Minutes)));
        let prose = typed("Transfer to a serving bowl and garnish with about 1/3 cup chives.");
        assert!(!prose.contains(&("1/3".into(), Quantity::Servings)));
        assert!(stated.contains(&("24".into(), Quantity::Servings)));
        assert!(stated.contains(&("4".into(), Quantity::Servings)));
        assert!(stated.contains(&("30".into(), Quantity::Minutes)));
        assert!(stated.contains(&("11".into(), Quantity::Minutes)));
        assert!(!stated.contains(&("30".into(), Quantity::Servings)));
        assert!(!stated.contains(&("4".into(), Quantity::Minutes)));
    }
}
