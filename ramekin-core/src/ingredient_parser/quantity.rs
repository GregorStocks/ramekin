//! Reads a parsed line's measurements into numbers, so nothing downstream
//! parses text. Each measurement becomes terms of an amount (or range) and a
//! unit in canonical form. Compound amounts ("1 cup plus 2 tbsp"), containers
//! that state their weight ("15-ounce can", "2 cans" noted "15-ounce"),
//! per-item weights ("2 cans, 10 oz each"), pieces cut to a size ("3-inch
//! piece") and weights stated only in the note ("4 (8 ounce packages)") are
//! all read here. The text is read with the measurement grammar's readers
//! (`measure.rs`), so amounts and sizes read the same here as when the line
//! was parsed. Nutrition weighs the result with the catalog's densities and
//! piece weights.

use std::sync::LazyLock;

use regex::Regex;

use super::measure::{self, Weighs};
use super::{Measurement, ParsedIngredient};
use crate::catalog::{normalize, volume_to_cups};

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

/// A whole string read as an amount: decimals, fractions, mixed numbers and
/// bounded ranges.
pub fn range(value: &str) -> Option<Range> {
    measure::read_range(value)
}

/// "2 (15-ounce) cans" parses as unit "can" with the weight leading the note
/// ("15-ounce, drained"); together they weigh one can. A per-item weight
/// reads the same ("3 packages (8 oz. each)" has note "8 oz. each"). None
/// when the note states no weight, and Some(None) when it states one that
/// doesn't read ("about 15-ounce").
fn noted_package(unit: &str, note: Option<&str>) -> Option<Option<Unit>> {
    if !measure::is_package_word(unit) {
        return None;
    }
    let (weight, mass) = measure::read_noted_package(&normalize(note?))?;
    Some(weight.map(|weight| Unit::Grams(weight.times(Range::exact(grams_per(&mass))))))
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

/// A normalized unit with any fill word dropped (a heaped or scant spoon is
/// close enough to a level one, and a packed cup to a plain one, as "1 cup
/// packed" already reads) and volume names in the catalog's spelling
/// ("tablespoons" -> "tbsp").
fn canonical_unit(normalized: &str) -> String {
    static FILL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"^(?:(?:slightly )?(?:heaped|heaping|scant|generous|level|rounded)|(?:firmly |lightly |loosely )?packed) ",
        )
        .unwrap()
    });
    let unit = FILL.replace(normalized, "");
    match unit.as_ref() {
        "c" | "cups" => "cup",
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
    if let Some((weight, mass)) = measure::read_package(normalized) {
        return Unit::Grams(weight.times(Range::exact(grams_per(&mass))));
    }
    if let Some(size) = measure::read_inch_piece(normalized) {
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

/// An amount in one unit, or a compound amount ("1 cup plus 2 tbsp", or
/// "1 plus 2" with a shared outer unit). Compound measurements embed their
/// units in the amount, with no outer unit; a shared outer unit instead
/// applies to every numeric term.
fn terms(amount: &str, outer: &Unit) -> Measure {
    measure::read_terms(&normalize(amount))?
        .into_iter()
        .map(|(amount, embedded)| {
            let unit = match embedded.as_str() {
                "" => outer.clone(),
                _ if *outer != Unit::Piece(String::new()) => return None,
                embedded => unit(embedded),
            };
            Some(Term { amount, unit })
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
    terms(measurement.amount.as_deref()?, &unit(&outer))
}

/// Whether a measurement can size one item: a plain mass, volume, or length
/// ("10 oz", "1 cup", "3 inches"). Counts of anything, weighed containers
/// ("3 8-ounce packages") and cut pieces ("2 3-inch chunks") included, can't.
pub(super) fn sizes_one_item(measurement: &Measurement) -> bool {
    let Some(unit) = measurement.unit.as_deref().map(normalize) else {
        return false;
    };
    if measure::read_package(&unit).is_some() || measure::read_inch_piece(&unit).is_some() {
        return false;
    }
    let unit = canonical_unit(&unit);
    mass_grams(&unit).is_some()
        || volume_to_cups(1.0, &unit).is_some()
        || matches!(unit.as_str(), "inch" | "inches" | "in" | "cm")
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
/// each", "8 ounce packages") or reads as an adjective ("4 (7- to 9-ounce)
/// potatoes"), and the whole line when it says so ("1 pound total"), is
/// hedged ("about 1 pound"), or the line is one item. Otherwise ("2 chicken
/// breasts" noted "8 oz") it could be either, so it weighs nothing.
fn noted_weight(ingredient: &ParsedIngredient) -> Option<Vec<Term>> {
    let note = normalize(ingredient.note.as_deref()?);
    let noted = note
        .split([',', ';'])
        .find_map(|part| measure::read_noted_weight(part.trim()))?;
    let weight = vec![Term {
        amount: noted.amount,
        unit: Unit::Grams(Range::exact(grams_per(&noted.mass))),
    }];
    let count = ingredient.measurements.first().and_then(count);
    match noted.weighs {
        Weighs::Each => Some(per_item(count?, weight)),
        Weighs::Total => Some(weight),
        Weighs::Unsaid => count
            .is_none_or(|count| count == Range::exact(1.0))
            .then_some(weight),
    }
}

/// A size left at the front of a counted item, sizing each of the count: "2
/// 4-ounce balls burrata", "1 14-oz block tofu", "1 2\" piece ginger".
fn leading_size(ingredient: &ParsedIngredient) -> Option<Vec<Term>> {
    let count = match read(ingredient.measurements.first()?)?.as_slice() {
        [Term {
            amount,
            unit: Unit::Piece(unit),
        }] if unit.is_empty() => *amount,
        _ => return None,
    };
    let (size, _) = super::split_leading_size(&ingredient.item)?;
    let size = normalize(size);
    let unit = match measure::read_leading_weight(&size) {
        Some((weight, mass)) => Unit::Grams(weight.times(Range::exact(grams_per(&mass)))),
        None => Unit::Inches(measure::read_inch_piece(
            &size.replace(['"', '”'], " inch"),
        )?),
    };
    Some(vec![Term {
        amount: count,
        unit,
    }])
}

/// The ways to measure a line, best first: per-item weights ("2 cans, 10 oz
/// each") and sizes leading the item ("2 4-ounce balls burrata"), then its
/// measurements in order (the primary leads, so rounded gram enrichments
/// don't replace precise amounts), then a weight stated only in the note.
/// Measurements are alternatives, not additive.
pub fn measures(ingredient: &ParsedIngredient) -> Vec<Measure> {
    let mut measures: Vec<Measure> = leading_size(ingredient).into_iter().map(Some).collect();
    let mut weighed_package = false;
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
                    .and_then(|amount| terms(amount, &unit(&each)))
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
        measures.push(match noted_package(&unit, ingredient.note.as_deref()) {
            Some(weighed) => {
                weighed_package = true;
                weighed.and_then(|unit| terms(amount, &unit))
            }
            None => terms(amount, &self::unit(&unit)),
        });
    }
    if !weighed_package {
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
        assert_eq!(range(",5"), Some(Range::exact(0.5)));
        assert_eq!(range("1,5–2,5"), Some(Range { min: 1.5, max: 2.5 }));
        assert_eq!(range("1-1/2-2"), Some(Range { min: 1.5, max: 2.0 }));
        assert_eq!(range("80/20 or 85/15").map(|r| r.min), Some(4.0));
    }

    #[test]
    fn units() {
        assert_eq!(unit("heaping tablespoons"), Unit::Cups(1.0 / 16.0));
        assert_eq!(unit("packed cup"), Unit::Cups(1.0));
        assert_eq!(unit("firmly packed tsp"), Unit::Cups(1.0 / 48.0));
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
        assert_eq!(
            unit("14 1/2-ounce can"),
            Unit::Grams(Range::exact(14.5 * 28.349523125))
        );
        assert_eq!(
            unit("(28-oz.) can"),
            Unit::Grams(Range::exact(28.0 * 28.349523125))
        );
        assert_eq!(unit("400g cans"), Unit::Grams(Range::exact(400.0)));
        // "1- inch knob" is a typo for "1-inch knob", as the line grammar reads it.
        assert_eq!(unit("3- inch chunk"), Unit::Inches(Range::exact(3.0)));
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
        assert_eq!(cups(terms("1 plus 2", &unit("cup")).unwrap()), 3.0);
        assert_eq!(cups(terms("1 cup plus 2 tbsp", &unit("")).unwrap()), 1.125);
        // The parser leaves an abbreviated unit inside a compound amount.
        assert_eq!(
            cups(terms("1/2 c plus 2 tablespoons", &unit("")).unwrap()),
            0.625
        );
        assert_eq!(terms("1 cup plus 2 tbsp", &unit("cup")), None);
    }

    #[test]
    fn noted_package_weights_join_the_unit() {
        let cans = measures(&line("2", Some("cans"), Some("15-ounce, drained")));
        assert_eq!(cans.len(), 1);
        close(grams(cans[0].as_ref().unwrap()), 850.49, 850.49);
        // Mixed-number and ranged sizes, as the parser spells them.
        let tomatoes = measures(&line("1", Some("can"), Some("14 1/2-ounce")));
        close(grams(tomatoes[0].as_ref().unwrap()), 411.07, 411.07);
        let squash = measures(&line("2", None, Some("1 1/2- to 1 3/4-lb")));
        close(grams(squash[1].as_ref().unwrap()), 1360.78, 1587.57);
    }

    #[test]
    fn noted_package_weights_must_read() {
        // A weight that doesn't read is an unclear amount, not a standard can.
        assert_eq!(
            measures(&line("1", Some("box"), Some("about 9 oz, 12 sheets"))),
            vec![None]
        );
        // A mass unit is a whole word: "seasoning" doesn't end in grams.
        assert_eq!(
            measures(&line("1", Some("jar"), Some("rice seasoning"))),
            vec![Some(vec![Term {
                amount: Range::exact(1.0),
                unit: Unit::Piece("jar".to_string())
            }])]
        );
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
    fn sizes_leading_the_item_lead() {
        let sized = |amount: &str, item: &str| {
            let mut line = line(amount, None, None);
            line.item = item.to_string();
            measures(&line)
        };
        let burrata = sized("2", "4-ounce balls burrata cheese");
        assert_eq!(burrata.len(), 2);
        close(grams(burrata[0].as_ref().unwrap()), 226.80, 226.80);
        let tofu = sized("1", "14-oz block extra-firm tofu");
        close(grams(tofu[0].as_ref().unwrap()), 396.89, 396.89);
        let goat_cheese = sized("1", "5- to 5 1/2-ounce log soft fresh goat cheese");
        close(grams(goat_cheese[0].as_ref().unwrap()), 141.75, 155.92);
        let milk = sized("2", "14-Ounce Each Cans Sweetened Condensed Milk");
        close(grams(milk[0].as_ref().unwrap()), 793.79, 793.79);
        let pudding = sized("2", "3.4 oz instant pistachio pudding mix");
        close(grams(pudding[0].as_ref().unwrap()), 192.78, 192.78);
        let rice = sized("2", "1-kg bags rice");
        close(grams(rice[0].as_ref().unwrap()), 2000.0, 2000.0);
        let sauce = sized("1", "15-ounce tomato sauce");
        close(grams(sauce[0].as_ref().unwrap()), 425.24, 425.24);
        assert_eq!(
            sized("1", "1 1/2\" piece ginger")[0],
            Some(vec![Term {
                amount: Range::exact(1.0),
                unit: Unit::Inches(Range::exact(1.5))
            }])
        );
        // Thick slices are no length of the food, and a size that leads no
        // food ("1-inch slices from a loaf") isn't split off.
        assert_eq!(sized("3", "1/2-inch-thick slices of ginger").len(), 1);
        assert_eq!(sized("4", "1-inch slices from a large loaf").len(), 1);
        // Only a bare count is a count of the sized items.
        let mut cups = line("2", Some("cups"), None);
        cups.item = "15-ounce tomato sauce".to_string();
        assert_eq!(measures(&cups).len(), 1);
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
        // A bare weight on one item is that item's.
        let steak = measures(&line("1", None, Some("16 oz")));
        close(grams(steak[1].as_ref().unwrap()), 453.59, 453.59);
        // On several, it could be each or all of them, so it weighs nothing.
        assert_eq!(measures(&line("2", None, Some("8 oz"))).len(), 1);
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
