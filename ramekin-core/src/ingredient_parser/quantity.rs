//! Reads a parsed line's measurements into numbers, so nothing downstream
//! parses text. Each measurement becomes terms of an amount (or range) and a
//! unit in canonical form. Compound amounts ("1 cup plus 2 tbsp"), containers
//! that state their weight ("15-ounce can", "2 cans" noted "15-ounce"),
//! per-item weights ("2 cans, 10 oz each"), pieces cut to a size ("3-inch
//! piece") and weights stated only in the note ("4 (8 ounce packages)") are
//! all read here. Nutrition weighs the result with the catalog's densities and
//! piece weights.

use std::sync::LazyLock;

use regex::Regex;

use super::fractions::unicode_fraction_ascii;
use super::{Measurement, ParsedIngredient};
use crate::catalog::{normalize, volume_to_cups};
use crate::metric_weights::parse_amount;

/// A number, or a bounded range of them ("2-3", "4 to 6").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    pub min: f64,
    pub max: f64,
}

impl Range {
    pub fn exact(value: f64) -> Self {
        Range {
            min: value,
            max: value,
        }
    }

    /// Both ends multiplied: the fewest of the smallest to the most of the
    /// largest.
    pub fn times(self, other: Range) -> Self {
        Range {
            min: self.min * other.min,
            max: self.max * other.max,
        }
    }
}

/// What one of a term's amount is.
#[derive(Debug, Clone, PartialEq)]
pub enum Unit {
    /// A mass, or a container that states its weight: grams in one.
    Grams(Range),
    /// A volume: cups in one.
    Cups(f64),
    /// A piece cut to a size, this many inches ("3-inch piece").
    Inches(Range),
    /// Anything else counts pieces, spelled as the catalog's piece weights
    /// look it up ("can", "large", "leaves"). Empty for a bare count.
    Piece(String),
}

/// An amount of one unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Term {
    pub amount: Range,
    pub unit: Unit,
}

/// One way to measure a line: its terms added together ("1 cup plus 2
/// tbsp"), or None when its amount isn't one this module can read.
pub type Measure = Option<Vec<Term>>;

/// Strict quantity grammar: decimals, fractions, mixed numbers and bounded
/// ranges. This does not alter the extraction pipeline's interpretation of
/// ingredients.
pub fn range(value: &str) -> Option<Range> {
    static GROUPED: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[1-9][0-9]{0,2},[0-9]{3}$").unwrap());
    static MIXED: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^([0-9]+)-([0-9]+/[0-9]+)$").unwrap());
    static NUMBER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+|[0-9]+/[0-9]+|[0-9]+ [0-9]+/[0-9]+)$")
            .unwrap()
    });
    let value: String = value
        .trim()
        .chars()
        .map(|c| match unicode_fraction_ascii(c) {
            Some(fraction) => format!(" {fraction}"),
            None => c.to_string(),
        })
        .collect();
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let number = |s: &str| {
        let s = MIXED.replace(s.trim(), "$1 $2");
        if GROUPED.is_match(&s) {
            return None;
        }
        let s = s.replace(',', ".");
        if !NUMBER.is_match(&s) {
            return None;
        }
        parse_amount(&s).filter(|n| n.is_finite() && *n >= 0.0 && *n <= 1e12)
    };
    if let Some(amount) = number(&value) {
        return Some(Range::exact(amount));
    }
    for delimiter in [" to ", " or ", "–", "—", "-"] {
        for (index, _) in value.match_indices(delimiter) {
            if let (Some(min), Some(max)) = (
                number(
                    value
                        .get(..index)
                        .expect("regex match starts at a character boundary"),
                ),
                number(
                    value
                        .get(index + delimiter.len()..)
                        .expect("regex match ends at a character boundary"),
                ),
            ) {
                if min <= max {
                    return Some(Range { min, max });
                }
            }
        }
    }
    None
}

/// Containers whose weight a recipe may state ("15-ounce can").
const PACKAGE_WORDS: &str = "can|package|block|bag|jar|box|carton|container|tin|bottle|packet|tub";

/// Mass units a package weight may use.
const PACKAGE_MASS: &str = "ounces?|oz|pounds?|lbs?|grams?|g";

/// "2 (15-ounce) cans" parses as unit "can" with the weight leading the note
/// ("15-ounce, drained"); join them back into one weighed unit. A per-item
/// weight reads the same ("3 packages (8 oz. each)" has note "8 oz. each").
fn noted_package_unit(unit: &str, note: Option<&str>) -> Option<String> {
    static BARE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(&format!(r"^(?:{PACKAGE_WORDS})s?$")).unwrap());
    static WEIGHT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(&format!(
            r"^(.+?)[- ]?({PACKAGE_MASS})\.?(?:$|[,;]| or | each\b)"
        ))
        .unwrap()
    });
    if !BARE.is_match(unit) {
        return None;
    }
    let note = normalize(note?);
    let weight = WEIGHT.captures(&note)?;
    Some(format!("{} {} {unit}", &weight[1], &weight[2]))
}

/// Grams in one of a mass unit.
fn grams_per(mass: &str) -> f64 {
    match mass.trim_end_matches('s') {
        "ounce" | "oz" => 28.349523125,
        "pound" | "lb" => 453.59237,
        "kilogram" | "kg" => 1000.0,
        "milligram" | "mg" => 0.001,
        _ => 1.0,
    }
}

/// A unit that carries its own weight: "15-ounce can", "(28-oz.) can", "14
/// 1/2-ounce can", "12- to 18-ounce package", "425-gram package".
fn package_grams(unit: &str) -> Option<Range> {
    static PACKAGE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(&format!(
            r"^\(?(.+?)[- ]?({PACKAGE_MASS})\.?\)?[- ]?(?:{PACKAGE_WORDS})s?$"
        ))
        .unwrap()
    });
    let parts = PACKAGE.captures(unit)?;
    // "12- to 18-ounce" shares the unit between both ends of the range.
    let weight = range(&parts[1].replace("- to ", " to "))?;
    Some(weight.times(Range::exact(grams_per(&parts[2]))))
}

/// The size of a piece cut to a size in inches ("3-inch piece", "3-4 inch
/// chunk", "3/4 inch knob").
fn inch_piece_size(unit: &str) -> Option<Range> {
    static INCH_PIECE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(.+?)[- ]inch(?:es)? (?:piece|chunk|knob|hunk)s?$").unwrap()
    });
    range(&INCH_PIECE.captures(unit)?[1])
}

/// A normalized unit with any fill word dropped (a heaped or scant spoon is
/// close enough to a level one) and volume names in the catalog's spelling
/// ("tablespoons" -> "tbsp").
fn canonical_unit(normalized: &str) -> String {
    static FILL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?:slightly )?(?:heaped|heaping|scant|generous|level|rounded) ").unwrap()
    });
    let unit = FILL.replace(normalized, "");
    match unit.as_ref() {
        "cups" => "cup",
        "pints" => "pint",
        "quarts" => "quart",
        "gallons" => "gallon",
        "tablespoon" | "tablespoons" => "tbsp",
        "teaspoon" | "teaspoons" => "tsp",
        "fluid ounce" | "fluid ounces" | "fl oz" => "fl oz",
        "milliliter" | "milliliters" => "ml",
        "liter" | "liters" | "litre" | "litres" => "l",
        other => other,
    }
    .to_string()
}

/// Grams in one of a mass unit in canonical spelling.
fn mass_grams(unit: &str) -> Option<f64> {
    match unit {
        "gram" | "grams" | "g" | "kilogram" | "kilograms" | "kg" | "milligram" | "milligrams"
        | "mg" | "ounce" | "ounces" | "oz" | "pound" | "pounds" | "lb" | "lbs" => {
            Some(grams_per(unit))
        }
        _ => None,
    }
}

/// A normalized unit, read.
fn unit(normalized: &str) -> Unit {
    if normalized.is_empty() {
        return Unit::Piece(String::new());
    }
    if let Some(grams) = package_grams(normalized) {
        return Unit::Grams(grams);
    }
    if let Some(size) = inch_piece_size(normalized) {
        return Unit::Inches(size);
    }
    let unit = canonical_unit(normalized);
    if let Some(grams) = mass_grams(&unit) {
        return Unit::Grams(Range::exact(grams));
    }
    match volume_to_cups(1.0, &unit) {
        Some(cups) => Unit::Cups(cups),
        None => Unit::Piece(unit),
    }
}

/// An amount and its normalized unit, including compound amounts ("1 cup
/// plus 2 tbsp", or "1 cup plus 2 tbsp" with the units in the amount).
fn terms(amount: &str, outer: &str) -> Measure {
    static PLUS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+(?:plus|\+)\s+").unwrap());
    static EMBEDDED: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(.+?)\s+([a-z][a-z ]*)$").unwrap());
    let amount = normalize(amount);
    PLUS.split(&amount)
        .map(|segment| {
            if let Some(amount) = range(segment) {
                return Some(Term {
                    amount,
                    unit: unit(outer),
                });
            }
            // Compound measurements embed their units in amount, with no outer
            // unit. A shared outer unit instead applies to every numeric term.
            if !outer.is_empty() {
                return None;
            }
            let parts = EMBEDDED.captures(segment)?;
            Some(Term {
                amount: range(&parts[1])?,
                unit: unit(&parts[2]),
            })
        })
        .collect()
}

/// One measurement read on its own, with no note.
pub fn read(measurement: &Measurement) -> Measure {
    let outer = measurement
        .unit
        .as_deref()
        .map(normalize)
        .unwrap_or_default();
    terms(measurement.amount.as_deref()?, &outer)
}

/// How many items a measurement counts ("2 cans", "3 steaks", "4"), when
/// it's a plain count rather than a weight or a volume.
fn count(measurement: &Measurement) -> Option<Range> {
    match read(measurement)?.as_slice() {
        [Term {
            amount,
            unit: Unit::Piece(_) | Unit::Inches(_),
        }] => Some(*amount),
        _ => None,
    }
}

/// Every term times `count`: a per-item measure for that many items.
fn per_item(count: Range, terms: Vec<Term>) -> Vec<Term> {
    terms
        .into_iter()
        .map(|term| Term {
            amount: count.times(term.amount),
            ..term
        })
        .collect()
}

/// A weight stated only in the note: "4 (8 ounce packages) cream cheese",
/// "1 rib roast, approximately 8 to 10 pounds", "2 chicken breasts, about 1
/// pound". It weighs one of a counted line's items when it says so ("6 oz
/// each", "8 ounce packages"), reads as an adjective ("4 (7- to 9-ounce)
/// potatoes"), or is in parentheses right after the count ("2 (8 oz) chicken
/// breasts"); otherwise it's the whole line's weight.
fn noted_weight(ingredient: &ParsedIngredient) -> Option<Vec<Term>> {
    static WEIGHT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(&format!(
            r"^(?:(?:about|approximately|approx\.?|roughly|around)\s+)?(.+?)([- ]?)({PACKAGE_MASS}|kilograms?|kg)\.?(?:\s*\([^()]*\))?(?:\s+(each|total|(?:{PACKAGE_WORDS})s?))?$"
        ))
        .unwrap()
    });
    let note = normalize(ingredient.note.as_deref()?);
    let (amount, mass, per) = note.split([',', ';']).find_map(|segment| {
        let parts = WEIGHT.captures(segment.trim())?;
        let amount = range(&parts[1].replace("- to ", " to "))?;
        let per = match parts.get(4).map(|word| word.as_str()) {
            Some("total") => false,
            Some(_) => true,
            None => &parts[2] == "-" || follows_count_in_parentheses(ingredient),
        };
        Some((amount, grams_per(&parts[3]), per))
    })?;
    let weight = vec![Term {
        amount,
        unit: Unit::Grams(Range::exact(mass)),
    }];
    if !per {
        return Some(weight);
    }
    Some(per_item(count(ingredient.measurements.first()?)?, weight))
}

/// Whether the raw line puts a parenthetical right after its count ("2 (8
/// oz) chicken breasts").
fn follows_count_in_parentheses(ingredient: &ParsedIngredient) -> bool {
    let (Some(raw), Some(amount)) = (
        ingredient.raw.as_deref(),
        ingredient
            .measurements
            .first()
            .and_then(|primary| primary.amount.as_deref()),
    ) else {
        return false;
    };
    raw.trim_start()
        .strip_prefix(amount)
        .is_some_and(|rest| rest.trim_start().starts_with('('))
}

/// The ways to measure a line, best first: per-item weights ("2 cans, 10 oz
/// each"), then its measurements in order (the primary leads, so rounded gram
/// enrichments don't replace precise amounts), then a weight stated only in
/// the note. Measurements are alternatives, not additive.
pub fn measures(ingredient: &ParsedIngredient) -> Vec<Measure> {
    let mut measures = Vec::new();
    let mut noted_package = false;
    if let Some((primary, alternatives)) = ingredient.measurements.split_first() {
        // A stated weight per item beats a package's standard size, as a
        // noted package weight does.
        if let Some(count) = count(primary) {
            for alternative in alternatives {
                let Some(each) = alternative
                    .unit
                    .as_deref()
                    .map(normalize)
                    .and_then(|unit| unit.strip_suffix(" each").map(str::to_string))
                else {
                    continue;
                };
                if let Some(terms) = alternative
                    .amount
                    .as_deref()
                    .and_then(|amount| terms(amount, &each))
                {
                    measures.push(Some(per_item(count, terms)));
                }
            }
        }
    }
    for (index, Measurement { amount, unit }) in ingredient.measurements.iter().enumerate() {
        let Some(amount) = amount.as_deref() else {
            measures.push(None);
            continue;
        };
        let unit = unit.as_deref().map(normalize).unwrap_or_default();
        // A per-item amount is the whole amount only on the primary ("2 cans
        // each artichoke hearts"); as an alternative it is one of the count.
        let unit = match unit.strip_suffix(" each") {
            Some(_) if index > 0 => continue,
            Some(stripped) => stripped.to_string(),
            None => unit,
        };
        let noted = noted_package_unit(&unit, ingredient.note.as_deref());
        noted_package |= noted.is_some();
        measures.push(terms(amount, noted.as_deref().unwrap_or(&unit)));
    }
    if !noted_package {
        if let Some(weight) = noted_weight(ingredient) {
            measures.push(Some(weight));
        }
    }
    measures
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(amount: &str, unit: Option<&str>, note: Option<&str>) -> ParsedIngredient {
        ParsedIngredient {
            item: "food".to_string(),
            measurements: vec![Measurement {
                amount: Some(amount.to_string()),
                unit: unit.map(str::to_string),
            }],
            note: note.map(str::to_string),
            raw: None,
            section: None,
        }
    }

    fn grams(terms: &[Term]) -> Range {
        terms.iter().fold(Range::exact(0.0), |total, term| {
            let Unit::Grams(per) = term.unit else {
                panic!("not a weight: {term:?}");
            };
            let grams = term.amount.times(per);
            Range {
                min: total.min + grams.min,
                max: total.max + grams.max,
            }
        })
    }

    fn close(actual: Range, min: f64, max: f64) {
        assert!(
            (actual.min - min).abs() < 0.01 && (actual.max - max).abs() < 0.01,
            "{actual:?} != {min}-{max}"
        );
    }

    #[test]
    fn ranges() {
        assert_eq!(range("1 ½"), Some(Range::exact(1.5)));
        assert_eq!(range("1-1/2"), Some(Range::exact(1.5)));
        assert_eq!(range("4 to 6"), Some(Range { min: 4.0, max: 6.0 }));
        assert_eq!(range("2–3"), Some(Range { min: 2.0, max: 3.0 }));
        assert_eq!(range("1,5"), Some(Range::exact(1.5)));
        assert_eq!(range("1,000"), None);
        assert_eq!(range("6-4"), None);
        assert_eq!(range("about 2"), None);
    }

    #[test]
    fn units() {
        assert_eq!(unit("heaping tablespoons"), Unit::Cups(1.0 / 16.0));
        assert_eq!(unit("lbs"), Unit::Grams(Range::exact(453.59237)));
        assert_eq!(
            unit("12- to 18-ounce package"),
            Unit::Grams(Range {
                min: 12.0 * 28.349523125,
                max: 18.0 * 28.349523125
            })
        );
        assert_eq!(
            unit("3-4 inch chunks"),
            Unit::Inches(Range { min: 3.0, max: 4.0 })
        );
        assert_eq!(unit("large"), Unit::Piece("large".to_string()));
        assert_eq!(unit(""), Unit::Piece(String::new()));
    }

    #[test]
    fn compound_amounts() {
        let cups = |terms: Vec<Term>| -> f64 {
            terms
                .iter()
                .map(|term| match term.unit {
                    Unit::Cups(cups) => term.amount.min * cups,
                    _ => panic!("not a volume: {term:?}"),
                })
                .sum()
        };
        assert_eq!(cups(terms("1 plus 2", "cup").unwrap()), 3.0);
        assert_eq!(cups(terms("1 cup plus 2 tbsp", "").unwrap()), 1.125);
        assert_eq!(terms("1 cup plus 2 tbsp", "cup"), None);
    }

    #[test]
    fn noted_package_weights_join_the_unit() {
        let measures = measures(&line("2", Some("cans"), Some("15-ounce, drained")));
        assert_eq!(measures.len(), 1);
        close(grams(measures[0].as_ref().unwrap()), 850.49, 850.49);
    }

    #[test]
    fn per_item_alternatives_lead() {
        let mut steaks = line("2", Some("steaks"), None);
        steaks.measurements.push(Measurement {
            amount: Some("4".to_string()),
            unit: Some("oz each".to_string()),
        });
        let measures = measures(&steaks);
        assert_eq!(measures.len(), 2, "the each alternative isn't its own");
        close(grams(measures[0].as_ref().unwrap()), 226.80, 226.80);
        assert_eq!(
            measures[1],
            Some(vec![Term {
                amount: Range::exact(2.0),
                unit: Unit::Piece("steaks".to_string())
            }])
        );
    }

    #[test]
    fn weights_stated_only_in_the_note() {
        let cream_cheese = measures(&line("4", None, Some("8 ounce packages")));
        close(grams(cream_cheese[1].as_ref().unwrap()), 907.18, 907.18);
        let roast = measures(&line("1", None, Some("approximately 8 to 10 pounds")));
        close(grams(roast[1].as_ref().unwrap()), 3628.74, 4535.92);
        // A whole-line weight isn't multiplied by the count.
        let carrots = measures(&line(
            "6",
            Some("large"),
            Some("about 1 1/2 pounds, trimmed"),
        ));
        close(grams(carrots[1].as_ref().unwrap()), 680.39, 680.39);
        let breasts = measures(&line("2", None, Some("boneless, about 1 pound total")));
        close(grams(breasts[1].as_ref().unwrap()), 453.59, 453.59);
        let fillets = measures(&line("2", None, Some("6 oz each")));
        close(grams(fillets[1].as_ref().unwrap()), 340.19, 340.19);
        let chickens = measures(&line("2", None, Some("2 1/2 lbs (1.1 k) each, quartered")));
        close(grams(chickens[1].as_ref().unwrap()), 2267.96, 2267.96);
        // An adjective weighs one item.
        let potatoes = measures(&line("4", None, Some("7- to 9-ounce, unpeeled")));
        close(grams(potatoes[1].as_ref().unwrap()), 793.79, 1020.58);
        // So does a parenthetical right after the count.
        let mut breasts = line("2", None, Some("8 oz"));
        breasts.raw = Some("2 (8 oz) chicken breasts".to_string());
        close(
            grams(measures(&breasts)[1].as_ref().unwrap()),
            453.59,
            453.59,
        );
        // A per-item weight needs a count to multiply.
        assert_eq!(
            measures(&line("1", Some("cup"), Some("8 ounce packages"))).len(),
            1
        );
        // Other notes say nothing about weight.
        assert_eq!(
            measures(&line("1", None, Some("about 8 ounces after peeling"))).len(),
            1
        );
    }
}
