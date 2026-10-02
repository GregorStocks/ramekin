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
    // "homemade or store-bought chicken broth": where it comes from, not what.
    "homemade",
    "large",
    "lightly",
    "loosely",
    "medium",
    "minced",
    "organic",
    "packed",
    "peeled",
    // "raw pecan halves"; an exact name ("raw sugar") matches first.
    "raw",
    "roughly",
    "shredded",
    "skinless",
    "sliced",
    "small",
    "store-bought",
    "storebought",
    "thinly",
    "toasted",
    // "unsalted pecan halves"; an exact name ("unsalted butter") matches first.
    "unsalted",
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
        .flat_map(|(_, candidates)| candidates)
        .find(|name| matches!(resolve_single(name), Resolution::Entry { entry: e, .. } if std::ptr::eq(e, entry)))
}

/// An "x or y" name's "or" chunks, without a leading article, or None when it
/// offers no alternatives.
fn alternative_chunks(text: &str) -> Option<Vec<&str>> {
    let chunks: Vec<&str> = text
        .split(" or ")
        .map(|chunk| {
            let chunk = chunk.trim().trim_matches(',');
            chunk
                .strip_prefix("a ")
                .or_else(|| chunk.strip_prefix("an "))
                .unwrap_or(chunk)
        })
        .collect();
    (chunks.len() >= 2 && chunks.iter().all(|chunk| !chunk.is_empty())).then_some(chunks)
}

/// The trailing words an "or" chunk lends earlier alternatives, longest
/// first: "red wine vinegar" lends "wine vinegar", then "vinegar". A trailing
/// clause lends nothing ("grape tomatoes, sliced" lends "tomatoes"), and a
/// chunk with its own amount ("vanilla or half a vanilla bean") is a measured
/// alternative that lends no noun; an article is not an amount ("peanut or a
/// vegetable oil" shares "oil").
fn tails_of(chunk: &str) -> Vec<String> {
    const AMOUNT_WORDS: [&str; 6] = ["one", "two", "three", "half", "some", "several"];
    let chunk = chunk.split(", ").next().unwrap_or_default();
    let measured = chunk.starts_with(|c: char| c.is_ascii_digit())
        || AMOUNT_WORDS.contains(&chunk.split(' ').next().unwrap_or_default());
    if measured {
        return Vec::new();
    }
    let words: Vec<&str> = chunk.split(' ').collect();
    (1..words.len())
        .map(|start| words[start..].join(" "))
        .collect()
}

/// The names an "x or y" name's alternatives are tried as, in order, or None
/// when it offers no alternatives. Each "or" chunk's comma pieces, then the
/// chunk itself ("melted, unsalted butter, olive oil, or ghee"), each cleaned
/// (`clean_alternative`). An alternative may borrow the list's noun: the
/// trailing words of the nearest later chunk that has some ("corn or flour
/// tortillas" is corn tortillas; "chicken, vegetable or seafood broth or
/// stock" is chicken broth), longest first ("sherry or red wine vinegar" tries
/// sherry wine vinegar, then sherry vinegar), or a head noun written first
/// ("oil canola, olive, or ..." is canola oil, olive oil). A one-word
/// alternative, prep words aside, is usually an adjective, so it borrows first
/// ("fresh lemon or lime juice" is lemon juice); a longer one names its food
/// and is tried as written first ("white wine or white balsamic vinegar" is
/// white wine) unless it parallels the lender ("red wine or white wine
/// vinegar") or a later one says "other". Grouped by the alternative as
/// written, cleaned.
fn alternative_candidates(normalized: &str) -> Option<Vec<(String, Vec<String>)>> {
    let text = normalized.replace(" and/or ", " or ");
    let chunks = alternative_chunks(&text)?;
    // "oil canola, olive, ...": a head noun written before the first item.
    let first_piece = chunks[0].split(", ").next().unwrap_or_default();
    let leading = first_piece
        .split_once(' ')
        .filter(|_| chunks[0].contains(", "))
        .map(|(noun, rest)| (noun.to_string(), rest.to_string()));
    let mut candidates = Vec::new();
    for (index, chunk) in chunks.iter().enumerate() {
        // A list chunk before the last ("mushroom, vegetable, chicken, or beef
        // broth") ends in an item, not the list's noun.
        let trailing = chunks[index + 1..]
            .iter()
            .enumerate()
            .filter(|(offset, later)| {
                index + 1 + offset == chunks.len() - 1 || !later.contains(", ")
            })
            .map(|(_, later)| (*later, tails_of(later)))
            .find(|(_, tails)| !tails.is_empty())
            .unwrap_or_default();
        let (lender, trailing) = trailing;
        let lender = lender.split(", ").next().unwrap_or_default();
        let mut pieces: Vec<String> = chunk
            .split(", ")
            .map(|piece| piece.trim().to_string())
            .filter(|piece| !piece.is_empty() && piece != chunk)
            .collect();
        pieces.push(chunk.to_string());
        for raw in pieces {
            let piece = clean_alternative(&raw);
            let mut nouned = Vec::new();
            if let Some((noun, rest)) = &leading {
                if raw == first_piece {
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
            let one_word = piece
                .split(' ')
                .filter(|word| !LEADING_MODIFIERS.contains(word))
                .count()
                <= 1;
            // "white wine or other mild vinegar": "other" says the list
            // shares a noun, so even a longer alternative borrows it first.
            let other_follows = chunks[index + 1..]
                .iter()
                .any(|later| later.starts_with("other ") || later.starts_with("another "));
            // "red wine or white wine vinegar": the alternative parallels the
            // lender's words before the shared noun, so it means red wine
            // vinegar, not red wine.
            let parallel = trailing.iter().any(|tail| {
                lender
                    .strip_suffix(tail.as_str())
                    .and_then(|head| head.trim_end().split_once(' '))
                    .is_some_and(|(_, word)| {
                        !word.contains(' ') && piece.rsplit(' ').next() == Some(word)
                    })
            });
            let names = if one_word || other_follows || parallel {
                nouned.into_iter().chain([piece.clone()]).collect()
            } else {
                [piece.clone()].into_iter().chain(nouned).collect()
            };
            candidates.push((piece, names));
        }
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

/// Words that change what a food is ("cooked red lentils" are not dry
/// lentils; "full-fat greek yogurt" is not the catalog's nonfat one), so the
/// first alternative never drops them to find its food.
const STATE_WORDS: &[&str] = &[
    "baked",
    "boiled",
    "candied",
    "canned",
    "cooked",
    "dried",
    "dry",
    "fat-free",
    "fried",
    "frozen",
    "full-fat",
    "grilled",
    "instant",
    "low-fat",
    "lowfat",
    "nonfat",
    "pickled",
    "prepared",
    "reduced-fat",
    "roasted",
    "smoked",
    "sweetened",
    "unsweetened",
];

/// The first alternative, in `alternative_candidates` order, that names one
/// food. When none of the first-listed alternative's candidates resolve but
/// one ends in a known food, that food counts ("local honey or maple syrup" is
/// honey; "mixed cherry or grape tomatoes" is cherry tomatoes; "hot or mild
/// paprika" is paprika), preferring one that keeps an alternative's own word;
/// if the words dropped to reach it change what it is ("cooked red lentils or
/// cannellini beans"), nothing counts rather than a later alternative. Later
/// alternatives are passed over only when the first names no known food at
/// all ("country or sourdough bread").
fn first_alternative(normalized: &str) -> Option<(&'static Entry, String)> {
    let entry_of = |name: &str| match resolve_single(name) {
        Resolution::Entry { entry, .. } => Some(entry),
        _ => None,
    };
    let mut groups = alternative_candidates(normalized)?.into_iter();
    let (piece, candidates) = groups.next()?;
    if let Some(found) = candidates
        .iter()
        .find_map(|name| entry_of(name).map(|entry| (entry, name.clone())))
    {
        return Some(found);
    }
    // A direct match only: clause trimming a fragment misleads.
    let directly = |name: &str| match resolve_single(name) {
        Resolution::Entry {
            entry,
            via: Via::Exact | Via::Plural | Via::Modifiers | Via::LeadingModifiers,
        } => Some(entry),
        _ => None,
    };
    // Only the piece and its borrowed-noun forms, borrowed forms first ("mixed
    // cherry" finds cherry tomatoes before cherries).
    let own_words = piece.split(' ').count();
    let names: Vec<&String> = candidates
        .iter()
        .filter(|name| **name != piece && name.starts_with(piece.as_str()))
        .chain(std::iter::once(&piece))
        .collect();
    for name in &names {
        let words: Vec<&str> = name.split(' ').collect();
        for start in 1..own_words.min(words.len()) {
            let rest = words[start..].join(" ");
            if let Some(entry) = directly(&rest) {
                if words[..start].iter().any(|word| STATE_WORDS.contains(word)) {
                    return None;
                }
                return Some((entry, rest));
            }
        }
    }
    // A one-word alternative is an adjective ("hot or mild paprika", "country
    // or sourdough bread"): it may give way to the noun the last alternative
    // ends in, whole ("sesame or poppy seeds to sprinkle" is not sprinkles).
    if own_words == 1
        && !STATE_WORDS.contains(&piece.as_str())
        && !LEADING_MODIFIERS.contains(&piece.as_str())
    {
        let text = normalized.replace(" and/or ", " or ");
        let last = alternative_chunks(&text)?.last().copied()?;
        if let Some(noun) = tails_of(last).into_iter().next() {
            if let Some(entry) = directly(&noun) {
                return Some((entry, noun));
            }
        }
    }
    groups.find_map(|(_, candidates)| {
        candidates
            .into_iter()
            .find_map(|name| entry_of(&name).map(|entry| (entry, name)))
    })
}
