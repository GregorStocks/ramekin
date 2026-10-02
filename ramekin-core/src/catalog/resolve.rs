//! Turn a written ingredient name into a catalog entry.

use std::sync::LazyLock;

use regex::Regex;

use super::{normalize, Entry, Target, CATALOG};

/// What a written ingredient name refers to.
#[derive(Debug)]
pub enum Resolution {
    Entry {
        entry: &'static Entry,
        via: Via,
    },
    /// Several foods sharing one line and one amount ("salt and pepper").
    Compound(Vec<&'static Entry>),
    /// Not an ingredient at all: a leftover header ("for the sauce:") or a
    /// curated phrase like "to serve".
    NotFood,
    /// The name could mean several foods (e.g. "cheese"), so no attribute is
    /// reported for it.
    Ambiguous,
    Unresolved,
}

/// Which lookup step matched, for debugging and audits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Exact,
    Plural,
    /// After stripping temperature or preparation modifiers.
    Modifiers,
    /// After dropping trailing ", ..." clauses.
    Clause,
    /// After dropping leading size or preparation words.
    LeadingModifiers,
    /// Through a stored LLM answer for a name the catalog doesn't know.
    Learned,
    /// The first-listed alternative of "x or y" that names a food (see
    /// `chosen_alternative`).
    Alternative,
}

/// Temperature and preparation modifiers stripped before a second lookup.
const MODIFIERS_TO_STRIP: &[&str] = &[
    "room temperature ",
    "cold ",
    "warm ",
    "melted ",
    "softened ",
    ", softened",
    ", melted",
    ", cold",
    ", at room temperature",
    ", room temperature",
    ", chilled",
    ", sifted",
    ", to taste",
    " to taste",
];

fn strip_modifiers(name: &str) -> String {
    let mut result = name.to_string();
    for modifier in MODIFIERS_TO_STRIP {
        if let Some(stripped) = result.strip_prefix(modifier) {
            result = stripped.to_string();
        }
        if let Some(stripped) = result.strip_suffix(modifier) {
            result = stripped.to_string();
        }
    }
    result
}

/// Leading words that describe size or preparation, never which food it is.
/// Words that change the food are deliberately absent: "ground" (ground beef),
/// "dried" (dried apricots), "light" (light cream), "crushed" (crushed
/// tomatoes), "whole" (whole chicken).
const LEADING_MODIFIERS: &[&str] = &[
    // "baking walnuts" (sold for baking); "baking soda" resolves before any
    // word is dropped.
    "baking",
    "best",
    "boneless",
    "chopped",
    "coarsely",
    "cooled",
    "diced",
    "extra-large",
    "finely",
    "firmly",
    "fresh",
    "freshly",
    "good",
    "grated",
    "large",
    "lightly",
    "loosely",
    "medium",
    "minced",
    "organic",
    "packed",
    "peeled",
    "roughly",
    "shredded",
    "skinless",
    "sliced",
    "small",
    "thinly",
    "toasted",
];

/// The name without a leading measure the parser left in it ("8 tbsp
/// unsalted butter", "240 ml heavy cream", "cloves garlic", "head green
/// cabbage"): a count or unit never names the food.
fn without_leading_measure(name: &str) -> Option<String> {
    static MEASURE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"^(?:(?:one|a|an|[0-9][0-9./ ]*)\s*)?(?:heaping\s+|heaped\s+)?(?:(?:tbsps?|tsps?|tablespoons?|teaspoons?|cups?|oz|ounces?|ml|g|grams?|lbs?|pounds?|sticks?|cloves?|heads?|cans?|bunch(?:es)?|sprigs?|stalks?|packets?|packages?)\.?(?:\s*(?:/|or)\s*[0-9][0-9./ ]*\s*(?:g|grams?|ml|oz|ounces?|lbs?|pounds?)\.?)?\s+(?:of\s+)?|heaping\s+|heaped\s+)",
        )
        .unwrap()
    });
    let rest = MEASURE.replace(name, "");
    (rest.len() < name.len() && !rest.is_empty()).then(|| rest.into_owned())
}

/// The name with leading size/preparation words dropped one at a time,
/// including comma-separated ones: "grated fresh ginger" -> ["fresh ginger",
/// "ginger"]. Trying each step lets a more specific name ("fresh ginger") win
/// before a bare one ("ginger").
fn leading_strips(name: &str) -> Vec<String> {
    let words: Vec<&str> = name.split(' ').collect();
    let strippable = words
        .iter()
        .take_while(|word| LEADING_MODIFIERS.contains(&word.trim_end_matches(',')))
        .count();
    (1..=strippable)
        .filter_map(|count| words.get(count..))
        .map(|rest| rest.join(" "))
        .filter(|rest| !rest.is_empty())
        .collect()
}

/// Whether an entry is a dried or ground form ("spices, sage, ground", "dried
/// oregano"). Dropping "fresh", "chopped", or "grated" must not land on one,
/// since fresh and dried herbs differ in both density and calories.
fn is_dried_form(entry: &Entry) -> bool {
    entry.id.starts_with("spices, ") || entry.id.contains("dried") || entry.id.ends_with(", ground")
}

/// The name with trailing ", ..." or "; ..." clauses removed one at a time,
/// longest prefix first ("a, b; c" -> ["a, b", "a"]).
fn clause_prefixes(name: &str) -> Vec<&str> {
    let mut prefixes = Vec::new();
    let mut rest = name;
    while let Some(index) = rest.rfind([',', ';']) {
        rest = rest
            .get(..index)
            .expect("a matched ASCII delimiter is a character boundary")
            .trim_end();
        if rest.is_empty() {
            break;
        }
        prefixes.push(rest);
    }
    prefixes
}

/// Singular/plural spellings to try when the name itself is unknown. The
/// plain "s" forms come first, matching the original density lookup.
fn plural_variants(name: &str) -> Vec<String> {
    let mut variants = vec![format!("{name}s")];
    variants.extend(name.strip_suffix('s').map(str::to_string));
    variants.push(format!("{name}es"));
    variants.extend(name.strip_suffix("es").map(str::to_string));
    variants.extend(name.strip_suffix("ies").map(|stem| format!("{stem}y")));
    variants.extend(name.strip_suffix('y').map(|stem| format!("{stem}ies")));
    variants
}

/// Exact lookup, then singular/plural variants.
fn lookup(name: &str) -> Option<(Target, Via)> {
    if let Some(target) = CATALOG.index.get(name) {
        return Some((*target, Via::Exact));
    }
    plural_variants(name)
        .iter()
        .find_map(|variant| CATALOG.index.get(variant).copied())
        .map(|target| (target, Via::Plural))
}

/// Resolve one already-normalized name as a single food, trying progressively
/// looser spellings. Only a match to a specific food ends the search: an ambiguous match (e.g.
/// "cheese") still lets a later spelling name a specific food, and trimming a
/// clause never settles for a name that isn't a food.
fn resolve_single(normalized: &str) -> Resolution {
    let normalized = normalized.to_string();

    let mut candidates: Vec<(String, Option<Via>)> = vec![
        (normalized.clone(), None),
        (strip_modifiers(&normalized), Some(Via::Modifiers)),
    ];
    for prefix in clause_prefixes(&normalized) {
        candidates.push((prefix.to_string(), Some(Via::Clause)));
        candidates.push((strip_modifiers(prefix), Some(Via::Clause)));
    }
    let unmeasured = without_leading_measure(&normalized);
    let mut stripped_names = leading_strips(&normalized);
    if let Some(unmeasured) = &unmeasured {
        stripped_names.push(unmeasured.clone());
        stripped_names.push(strip_modifiers(unmeasured));
        stripped_names.extend(leading_strips(unmeasured));
        stripped_names.extend(leading_strips(&strip_modifiers(unmeasured)));
    }
    for stripped in stripped_names {
        let prefixes: Vec<String> = clause_prefixes(&stripped)
            .into_iter()
            .map(str::to_string)
            .collect();
        candidates.push((stripped, Some(Via::LeadingModifiers)));
        candidates.extend(
            prefixes
                .into_iter()
                .map(|prefix| (prefix, Some(Via::LeadingModifiers))),
        );
    }
    let says_dried = normalized.contains("dried") || normalized.contains("ground");

    let mut ambiguous = false;
    let mut tried = std::collections::HashSet::new();
    for (name, step) in candidates {
        if !tried.insert(name.clone()) {
            continue;
        }
        match lookup(&name) {
            Some((Target::Entry(entry), via)) => {
                let entry = &CATALOG.entries[entry];
                // A bare name reached by dropping "fresh" or "chopped" must not become
                // the dried spice; a curated alias that keeps a prep word (e.g.
                // "grated nutmeg") is an explicit choice and still counts.
                let bare = !name
                    .split(' ')
                    .any(|word| LEADING_MODIFIERS.contains(&word.trim_end_matches(',')));
                if step == Some(Via::LeadingModifiers)
                    && bare
                    && !says_dried
                    && is_dried_form(entry)
                {
                    continue;
                }
                return Resolution::Entry {
                    entry,
                    via: step.unwrap_or(via),
                };
            }
            Some((Target::Ambiguous, _)) => ambiguous = true,
            // Trimming a clause or leading words never settles for a non-food.
            Some((Target::NotFood, _)) if matches!(step, None | Some(Via::Modifiers)) => {
                return Resolution::NotFood;
            }
            Some((Target::NotFood, _)) | None => {}
        }
    }
    if ambiguous {
        Resolution::Ambiguous
    } else {
        Resolution::Unresolved
    }
}

/// Several foods joined by "and" or "&" in one ingredient name. Only tried
/// once the whole name failed, so "half and half" stays one food; every part
/// must resolve to a specific food on its own. "and/or" offers alternatives.
fn compound(normalized: &str) -> Option<Resolution> {
    if normalized.contains(" and/or ") {
        return None;
    }
    let joined = normalized.replace(" & ", " and ");
    let parts: Vec<&str> = joined.split(" and ").map(str::trim).collect();
    if parts.len() < 2 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    let entries = parts
        .iter()
        .map(|part| match resolve_single(part) {
            Resolution::Entry { entry, .. } => Some(entry),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Resolution::Compound(entries))
}

/// Resolve a written ingredient name, trying progressively looser spellings
/// and then, if it names no single food, several foods joined by "and". A name
/// ending in ":" is a leftover section header, not an ingredient.
pub fn resolve(item: &str) -> Resolution {
    let normalized = normalize(item);
    if normalized.ends_with(':') {
        return Resolution::NotFood;
    }
    resolve_name(&normalized)
}

/// One name as a single food, else as several joined by "and", else as the
/// first-listed alternative of "x or y".
fn resolve_name(normalized: &str) -> Resolution {
    // A name offering alternatives: a direct match ("butter or margarine" is a
    // curated alias) wins, then the first alternative, before clause trimming
    // can read ", grape, or cranberry juice" as a droppable clause.
    if alternative_candidates(normalized).is_some() {
        match resolve_single(normalized) {
            direct @ Resolution::Entry {
                via: Via::Exact | Via::Plural | Via::Modifiers,
                ..
            } => return direct,
            not_food @ Resolution::NotFood => return not_food,
            _ => {}
        }
        if let Some((entry, _)) = first_alternative(normalized) {
            return Resolution::Entry {
                entry,
                via: Via::Alternative,
            };
        }
    }
    match resolve_single(normalized) {
        unresolved @ (Resolution::Ambiguous | Resolution::Unresolved) => compound(normalized)
            .or_else(|| {
                first_alternative(normalized).map(|(entry, _)| Resolution::Entry {
                    entry,
                    via: Via::Alternative,
                })
            })
            .unwrap_or(unresolved),
        resolved => resolved,
    }
}

/// The alternative an "x or y" name ("mayonnaise or plain yogurt", "vegetable,
/// canola, or peanut oil") was counted as, for the estimate's "assumed" note:
/// the alternative step-7 chose, or for a name a curated alias resolves
/// ("butter or margarine"), the listed alternative naming that same food.
/// None for a name that offers no alternatives.
pub fn chosen_alternative(item: &str) -> Option<String> {
    let normalized = normalize(item);
    let (entry, via) = match resolve_name(&normalized) {
        Resolution::Entry { entry, via } => (entry, via),
        _ => return None,
    };
    if via == Via::Alternative {
        return first_alternative(&normalized).map(|(_, text)| text);
    }
    alternative_candidates(&normalized)?
        .into_iter()
        .find(|name| matches!(resolve_single(name), Resolution::Entry { entry: e, .. } if std::ptr::eq(e, entry)))
}

/// The names an "x or y" name's alternatives are tried as, in order, or None
/// when it offers no alternatives. Each "or" chunk's comma pieces, then the
/// chunk itself ("melted, unsalted butter, olive oil, or ghee"). Each is first
/// tried with the list's noun, then alone: the last alternative's trailing
/// words, longest first ("corn or flour tortillas" is corn tortillas, "fresh
/// lemon or lime juice" lemon juice, "sherry or red wine vinegar" tries sherry
/// wine vinegar, then sherry vinegar), or a leading noun ("oil canola, olive,
/// or ..." is canola oil, olive oil).
fn alternative_candidates(normalized: &str) -> Option<Vec<String>> {
    let text = normalized.replace(" and/or ", " or ");
    let chunks: Vec<&str> = text
        .split(" or ")
        .map(|chunk| chunk.trim().trim_matches(','))
        .collect();
    if chunks.len() < 2 || chunks.iter().any(|chunk| chunk.is_empty()) {
        return None;
    }
    // The last alternative's trailing words, longest first: "red wine vinegar"
    // gives "wine vinegar", then "vinegar".
    // A last alternative with its own amount ("vanilla or 1 vanilla bean",
    // "vanilla or half a vanilla bean") is a measured alternative, not a
    // shared noun.
    const AMOUNT_WORDS: [&str; 6] = ["one", "two", "three", "half", "some", "several"];
    // An article is not an amount: "peanut or a vegetable oil" shares "oil".
    let last = chunks[chunks.len() - 1];
    let last = last
        .strip_prefix("a ")
        .or_else(|| last.strip_prefix("an "))
        .unwrap_or(last);
    let measured = last.starts_with(|c: char| c.is_ascii_digit())
        || AMOUNT_WORDS.contains(&last.split(' ').next().unwrap_or_default());
    let last_words: Vec<&str> = if measured {
        Vec::new()
    } else {
        last.split(' ').collect()
    };
    let trailing: Vec<String> = (1..last_words.len())
        .map(|start| last_words[start..].join(" "))
        .collect();
    // "oil canola, olive, ...": a head noun written before the first item.
    let first_piece = chunks[0].split(", ").next().unwrap_or_default();
    let leading = first_piece
        .split_once(' ')
        .filter(|_| chunks[0].contains(", "))
        .map(|(noun, rest)| (noun.to_string(), rest.to_string()));
    // A chunk's comma pieces come before the whole chunk: resolving "apple,
    // grape" whole drops the clause and lands on apples, while the piece
    // "apple" with the shared noun is apple juice.
    let mut pieces = Vec::new();
    for chunk in &chunks {
        pieces.extend(
            chunk
                .split(", ")
                .map(|piece| piece.trim().to_string())
                .filter(|piece| !piece.is_empty() && piece != chunk),
        );
        pieces.push(chunk.to_string());
    }
    let mut candidates = Vec::new();
    for piece in pieces.iter().map(|piece| clean_alternative(piece)) {
        let mut nouned = Vec::new();
        if let Some((noun, rest)) = &leading {
            if piece == *first_piece {
                nouned.push(format!("{rest} {noun}"));
            } else if !piece.contains(' ') {
                nouned.push(format!("{piece} {noun}"));
            }
        }
        for tail in trailing
            .iter()
            .filter(|tail| !piece.ends_with(tail.as_str()))
        {
            nouned.push(format!("{piece} {tail}"));
        }
        // The shared noun first: "fresh lemon or lime juice" is lemon juice,
        // not lemons; the bare piece still follows ("sour cream or plain
        // yogurt").
        candidates.extend(nouned);
        candidates.push(piece);
    }
    Some(candidates)
}

/// One alternative without what keeps it from naming its food: an example
/// marker ("like cream cheese", "such as ...") and a prep word joined by "and"
/// ("cooked and cooled white rice" is cooked white rice).
fn clean_alternative(piece: &str) -> String {
    static EXAMPLE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?:[^:]+:\s*)?(?:(?:like|such as|e\.g\.|eg|for example),? )?").unwrap()
    });
    // A leading label ("berries: sliced strawberries") and an example marker.
    let piece = EXAMPLE.replace(piece, "");
    let words: Vec<&str> = piece.split(' ').collect();
    let mut kept = Vec::with_capacity(words.len());
    let mut index = 0;
    while index < words.len() {
        if words[index] == "and"
            && index > 0
            && words
                .get(index + 1)
                .is_some_and(|word| LEADING_MODIFIERS.contains(word))
        {
            index += 2;
            continue;
        }
        kept.push(words[index]);
        index += 1;
    }
    kept.join(" ")
}

/// The first candidate, in `alternative_candidates` order, that names one food.
fn first_alternative(normalized: &str) -> Option<(&'static Entry, String)> {
    alternative_candidates(normalized)?
        .into_iter()
        .find_map(|name| match resolve_single(&name) {
            Resolution::Entry { entry, .. } => Some((entry, name)),
            _ => None,
        })
}
