//! Unit tables, unit extraction (compound/attached/multiplier), and normalization.

use super::*;

/// Common cooking units (lowercase for matching).
/// Sorted by length at runtime (longest first) to avoid partial matches
/// (e.g., "tablespoons" must match before "tb").
pub(super) static UNITS_SORTED: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut units = UNITS_RAW.to_vec();
    units.sort_by_key(|u| std::cmp::Reverse(u.len()));
    units
});

pub(super) const UNITS_RAW: &[&str] = &[
    // Volume - US
    "fluid ounces",
    "fluid ounce",
    "tablespoons",
    "tablespoon",
    "teaspoons",
    "teaspoon",
    "gallons",
    "gallon",
    "quarts",
    "quart",
    "pints",
    "pint",
    "cups",
    "cup",
    "tbsp",
    "tbs",
    "tsp",
    "fl oz",
    "fl. oz",
    "gal",
    "qt",
    "pt",
    "tb",
    "ts",
    "c",
    // Volume - Metric
    "milliliters",
    "milliliter",
    "liters",
    "liter",
    "litres",
    "litre",
    "ml",
    "l",
    // Weight - US
    "ounces",
    "ounce",
    "pounds",
    "pound",
    "lbs",
    "lb",
    "oz",
    // Weight - Metric
    "kilograms",
    "kilogram",
    "milligrams",
    "milligram",
    "grams",
    "gram",
    "kg",
    "mg",
    "g",
    // Count/Size
    "packages",
    "package",
    "handfuls",
    "handful",
    "bottles",
    "bunches",
    "pinches",
    "slices",
    "sprigs",
    "stalks",
    "pieces",
    "bottle",
    "cloves",
    "dashes",
    "drops",
    "heads",
    "sticks",
    "bunch",
    "clove",
    "cubes",
    "piece",
    "pinch",
    "slice",
    "sprig",
    "stalk",
    "boxes",
    "cans",
    "jars",
    "bags",
    "cube",
    "dash",
    "drop",
    "head",
    "pkgs",
    "stick",
    "box",
    "can",
    "jar",
    "bag",
    "pcs",
    "pkg",
    "pc",
    // Counts of a container, a shape or a serving: "1 envelope taco
    // seasoning", "2 ears corn", "1 ball burrata". All but "splash" and
    // "dollop" need an amount (`needs_amount`), so a food that starts with
    // one ("bar cookies") keeps it.
    "envelopes",
    "envelope",
    "packets",
    "packet",
    "sachets",
    "sachet",
    "containers",
    "container",
    "cartons",
    "carton",
    "pouches",
    "pouch",
    "tins",
    "tin",
    "tubs",
    "tub",
    "packs",
    "pack",
    "blocks",
    "block",
    "loaves",
    "loaf",
    "sheets",
    "sheet",
    "bars",
    "bar",
    "balls",
    "ball",
    "ears",
    "ear",
    "leaves",
    "rashers",
    "rasher",
    "chunks",
    "chunk",
    "knobs",
    "knob",
    "wedges",
    "wedge",
    "fillets",
    "fillet",
    "bundles",
    "bundle",
    "batches",
    "batch",
    "portions",
    "portion",
    "scoops",
    "scoop",
    "dollops",
    "dollop",
    "splashes",
    "splash",
    "shots",
    "shot",
    // Size descriptors that act like units
    "extra-large",
    "medium",
    "small",
    "large",
    "xl",
];

/// Whether `unit` is only a unit after an amount: "1 bar chocolate" counts
/// bars, but "bar cookies" and "Leaves from 3 sprigs thyme" name the food.
pub(super) fn needs_amount(unit: &str) -> bool {
    let unit = normalize_unit(&unit.to_lowercase());
    COUNT_UNITS_NEEDING_AMOUNT.contains(&unit.as_str())
}

/// The canonical count units added for containers, shapes and servings,
/// except the amount-less "splash" and "dollop" ("splash of milk").
const COUNT_UNITS_NEEDING_AMOUNT: &[&str] = &[
    "envelope",
    "packet",
    "sachet",
    "container",
    "carton",
    "pouch",
    "tin",
    "tub",
    "pack",
    "block",
    "loaf",
    "sheet",
    "bar",
    "ball",
    "ear",
    "leaf",
    "rasher",
    "chunk",
    "knob",
    "wedge",
    "fillet",
    "bundle",
    "batch",
    "portion",
    "scoop",
    "shot",
];

/// Size descriptors that act like units ("1 large onion"), in canonical form.
const SIZE_UNITS: &[&str] = &["small", "medium", "large", "extra-large"];

pub(super) fn is_size_unit(unit: &str) -> bool {
    SIZE_UNITS.contains(&normalize_unit_base(unit.trim()).to_lowercase().as_str())
}

/// Map of unit variations to their canonical forms.
/// Used by normalize_unit() to standardize units after parsing.
pub(super) static UNIT_CANONICAL_MAP: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();

        // Volume - small
        map.insert("teaspoon", "tsp");
        map.insert("teaspoons", "tsp");
        map.insert("ts", "tsp");

        map.insert("tablespoon", "tbsp");
        map.insert("tablespoons", "tbsp");
        map.insert("tbs", "tbsp");
        map.insert("tb", "tbsp");

        // Volume - cups
        map.insert("cups", "cup");
        map.insert("c", "cup");

        // Volume - larger
        map.insert("pints", "pint");
        map.insert("pt", "pint");

        map.insert("quarts", "quart");
        map.insert("qt", "quart");

        map.insert("gallons", "gallon");
        map.insert("gal", "gallon");

        map.insert("fluid ounce", "fl oz");
        map.insert("fluid ounces", "fl oz");
        map.insert("fl. oz", "fl oz");

        // Volume - metric
        map.insert("milliliter", "ml");
        map.insert("milliliters", "ml");

        map.insert("liter", "l");
        map.insert("liters", "l");
        map.insert("litre", "l");
        map.insert("litres", "l");

        // Weight - US
        map.insert("ounce", "oz");
        map.insert("ounces", "oz");

        map.insert("pound", "lb");
        map.insert("pounds", "lb");
        map.insert("lbs", "lb");

        // Weight - metric
        map.insert("gram", "g");
        map.insert("grams", "g");

        map.insert("kilogram", "kg");
        map.insert("kilograms", "kg");

        map.insert("milligram", "mg");
        map.insert("milligrams", "mg");

        // Count/Container - normalize plurals to singular
        map.insert("cloves", "clove");
        map.insert("slices", "slice");
        map.insert("pieces", "piece");
        map.insert("pc", "piece");
        map.insert("pcs", "piece");
        map.insert("cans", "can");
        map.insert("jars", "jar");
        map.insert("bottles", "bottle");
        map.insert("bags", "bag");
        map.insert("boxes", "box");
        map.insert("packages", "package");
        map.insert("pkg", "package");
        map.insert("pkgs", "package");
        map.insert("sticks", "stick");
        map.insert("bunches", "bunch");
        map.insert("sprigs", "sprig");
        map.insert("pinches", "pinch");
        map.insert("dashes", "dash");
        map.insert("drops", "drop");
        map.insert("heads", "head");
        map.insert("stalks", "stalk");
        map.insert("handfuls", "handful");
        map.insert("cubes", "cube");
        for (plural, singular) in [
            ("envelopes", "envelope"),
            ("packets", "packet"),
            ("sachets", "sachet"),
            ("containers", "container"),
            ("cartons", "carton"),
            ("pouches", "pouch"),
            ("tins", "tin"),
            ("tubs", "tub"),
            ("packs", "pack"),
            ("blocks", "block"),
            ("loaves", "loaf"),
            ("sheets", "sheet"),
            ("bars", "bar"),
            ("balls", "ball"),
            ("ears", "ear"),
            ("leaves", "leaf"),
            ("rashers", "rasher"),
            ("chunks", "chunk"),
            ("knobs", "knob"),
            ("wedges", "wedge"),
            ("fillets", "fillet"),
            ("bundles", "bundle"),
            ("batches", "batch"),
            ("portions", "portion"),
            ("scoops", "scoop"),
            ("dollops", "dollop"),
            ("splashes", "splash"),
            ("shots", "shot"),
        ] {
            map.insert(plural, singular);
        }

        // Size - normalize xl
        map.insert("xl", "extra-large");

        map
    });

/// Measurement modifiers that appear before amounts or between amounts and units.
/// These are stripped during parsing but preserved in the raw field.
/// Examples: "scant 1 teaspoon", "2 heaping tablespoons", "1 generous cup"
pub(super) const MEASUREMENT_MODIFIERS: &[&str] = &[
    // Multi-word modifiers first (longer matches take priority)
    "lightly packed",
    "firmly packed",
    "loosely packed",
    "slightly heaped",
    "slightly heaping",
    // Single-word modifiers
    "scant",
    "heaping",
    "heaped",
    "rounded",
    "level",
    "generous",
    "good",
    "packed",
];

/// Strip measurement modifiers from the beginning of a string.
/// Returns (modifier if found, remaining_string).
pub(super) fn strip_measurement_modifier(s: &str) -> (Option<String>, String) {
    let s_lower = s.to_lowercase();
    let s_trimmed = s.trim();

    for &modifier in MEASUREMENT_MODIFIERS {
        if s_lower.trim().starts_with(modifier) {
            if let Some(after) = s_trimmed.get(modifier.len()..) {
                // "good quality chocolate" names how good the food is, not a
                // generous measure.
                let names_quality = after
                    .trim_start()
                    .get(..7)
                    .is_some_and(|word| word.eq_ignore_ascii_case("quality"));
                // Make sure it's a word boundary (followed by space or end)
                if (after.is_empty() || after.starts_with(char::is_whitespace)) && !names_quality {
                    return (Some(modifier.to_string()), after.trim().to_string());
                }
            }
        }
    }

    (None, s_trimmed.to_string())
}

pub(super) fn units_share_base(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
        || a.strip_suffix(&format!(" {}", b))
            .is_some_and(|prefix| !prefix.is_empty())
        || b.strip_suffix(&format!(" {}", a))
            .is_some_and(|prefix| !prefix.is_empty())
}

pub(super) fn extract_unit(s: &str) -> (Option<String>, String) {
    measure::read_unit(s)
}

/// Count/container nouns that pair with a hyphenated size descriptor.
/// Examples: "14-ounce package", "1/2-inch piece", "8-ounce block".
pub(super) const HYPHENATED_DESCRIPTOR_NOUNS: &[&str] = &[
    "package", "packages", "pkg", "pkgs", "can", "cans", "bag", "bags", "block", "blocks", "wheel",
    "wheels", "piece", "pieces", "knob", "knobs", "segment", "segments", "slice", "slices",
    "stick", "sticks", "loaf", "loaves", "hunk", "hunks",
];

/// Container types that can form compound units like "14 ounce can"
pub(super) const CONTAINERS: &[&str] = &[
    "packages",
    "package",
    "bottles",
    "bottle",
    "boxes",
    "cans",
    "jars",
    "bags",
    "box",
    "can",
    "jar",
    "bag",
    "pkgs",
    "pkg",
    "containers",
    "container",
];

/// A container or packet that holds the food ("can", "packet", "envelope").
pub(super) fn is_package_noun(word: &str) -> bool {
    let lower = word.to_lowercase();
    CONTAINERS.contains(&lower.as_str())
        || [
            "packet",
            "packets",
            "envelope",
            "envelopes",
            "sachet",
            "sachets",
        ]
        .contains(&lower.as_str())
}

/// Nouns for a piece cut to a size in inches ("3-inch piece", "3-4 inch chunk").
const INCH_PIECE_NOUNS: &[&str] = &[
    "piece", "pieces", "chunk", "chunks", "knob", "knobs", "hunk", "hunks",
];

/// A size in inches before a piece noun, as the unit: "3-inch piece",
/// "3-4 inch chunk", "1 1/2-inch knob". The size is a number or a range.
fn try_extract_inch_piece_unit(words: &[&str]) -> Option<(String, String)> {
    let is_size = |size: &str| !size.is_empty() && size.split('-').all(is_amount_like);
    // "1 1/2-inch knob": a whole number before a fraction is one size.
    let size_index = usize::from(
        words.first()?.chars().all(|c| c.is_ascii_digit())
            && words
                .get(1)
                .and_then(|word| word.split('-').next())
                .is_some_and(is_fraction),
    );
    let size_lower = words.get(size_index)?.to_lowercase();
    let noun_index = match size_lower.strip_suffix("-inch") {
        Some(size) if is_size(size) => size_index + 1,
        _ if is_size(&size_lower)
            && words
                .get(size_index + 1)
                .is_some_and(|word| ["inch", "inches"].contains(&word.to_lowercase().as_str())) =>
        {
            size_index + 2
        }
        _ => return None,
    };
    let noun = words.get(noun_index)?;
    if !INCH_PIECE_NOUNS.contains(&noun.to_lowercase().as_str()) {
        return None;
    }
    let mut rest = &words[noun_index + 1..];
    if rest
        .first()
        .is_some_and(|word| word.eq_ignore_ascii_case("of"))
    {
        rest = &rest[1..];
    }
    Some((words[..=noun_index].join(" "), rest.join(" ")))
}

/// True if a parenthetical is only an inch size and a piece noun follows it:
/// "(1 1/2-inch) piece", "(2 inch) chunks".
pub(super) fn is_inch_size_before_piece(size: &str, after: &str) -> bool {
    let Some(noun) = after.split_whitespace().next() else {
        return false;
    };
    let mut words: Vec<&str> = size.split_whitespace().collect();
    words.push(noun);
    try_extract_inch_piece_unit(&words).is_some_and(|(_, rest)| rest.is_empty())
}

/// Weight/volume units that can precede containers in compound units
pub(super) const WEIGHT_UNITS_FOR_COMPOUND: &[&str] = &[
    "ounce",
    "ounces",
    "oz",
    "gram",
    "grams",
    "g",
    "pound",
    "pounds",
    "lb",
    "lbs",
    "ml",
    "milliliter",
    "milliliters",
    "liter",
    "liters",
    "l",
];

/// Metric units that can be attached to numbers without space (e.g., "65g", "100ml")
pub(super) const ATTACHED_METRIC_UNITS: &[&str] = &["kg", "g", "mg", "ml", "l", "oz", "lb", "lbs"];

/// Normalize a unit string to its canonical form.
///
/// Handles:
/// - Direct mappings: "cups" → "cup", "tablespoons" → "tbsp"
/// - Modifiers: "heaping cups" → "heaping cup"
/// - "each" suffix: "ounces each" → "oz each"
///
/// Returns the original unit if no normalization is needed.
pub(super) fn normalize_unit(unit: &str) -> String {
    let unit = unit.trim();
    if unit.is_empty() {
        return unit.to_string();
    }

    // Check for "each" suffix first (case-insensitive check, preserve original for base)
    let (base_unit, each_suffix) = if unit.to_lowercase().ends_with(" each") {
        // Safe because " each" is 5 ASCII bytes
        (unit.get(..unit.len() - 5).unwrap_or(unit), " each")
    } else {
        (unit, "")
    };

    // Check if there's a modifier prefix (from MEASUREMENT_MODIFIERS)
    let base_lower = base_unit.to_lowercase();
    for &modifier in MEASUREMENT_MODIFIERS {
        if base_lower.starts_with(modifier) {
            let after_modifier = base_unit.get(modifier.len()..).unwrap_or("").trim();
            if !after_modifier.is_empty() {
                // Normalize the unit part after the modifier
                let normalized_base = normalize_unit_base(after_modifier);
                return format!("{} {}{}", modifier, normalized_base, each_suffix);
            }
        }
    }

    // No modifier, just normalize the base unit
    let normalized = normalize_unit_base(base_unit);
    format!("{}{}", normalized, each_suffix)
}

/// Normalize a base unit (without modifiers) using UNIT_CANONICAL_MAP.
pub(super) fn normalize_unit_base(unit: &str) -> String {
    let unit_lower = unit.to_lowercase();
    if let Some(&canonical) = UNIT_CANONICAL_MAP.get(unit_lower.as_str()) {
        canonical.to_string()
    } else {
        unit.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hyphenated_compound_unit_tail_package() {
        let result = parse_ingredient("14-ounce package extra-firm tofu");
        assert_eq!(result.item, "extra-firm tofu");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(
            result.measurements[0].unit,
            Some("14-ounce package".to_string())
        );
    }

    #[test]
    fn test_hyphenated_compound_unit_tail_fractional_piece() {
        let result = parse_ingredient("1/2-inch piece of fresh ginger");
        assert_eq!(result.item, "fresh ginger");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(
            result.measurements[0].unit,
            Some("1/2-inch piece".to_string())
        );
    }

    #[test]
    fn test_hyphenated_unit_tail_without_descriptor_noun() {
        let result = parse_ingredient("1-pound ground beef");
        assert_eq!(result.item, "ground beef");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].amount, Some("1".to_string()));
        assert_eq!(result.measurements[0].unit, Some("lb".to_string()));
    }
}
