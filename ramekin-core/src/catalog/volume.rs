//! Volume units the catalog's densities (grams per US cup) can convert.

pub const CUPS_PER_TBSP: f64 = 1.0 / 16.0;
pub const CUPS_PER_TSP: f64 = 1.0 / 48.0;
pub const CUPS_PER_FL_OZ: f64 = 1.0 / 8.0;
pub const CUPS_PER_PINT: f64 = 2.0;
pub const CUPS_PER_QUART: f64 = 4.0;
pub const CUPS_PER_GALLON: f64 = 16.0;
pub const CUPS_PER_ML: f64 = 1.0 / 236.588;
pub const CUPS_PER_L: f64 = 1000.0 / 236.588;

/// Convert an amount of a canonical volume unit to cups.
pub fn volume_to_cups(amount: f64, unit: &str) -> Option<f64> {
    match unit {
        "cup" => Some(amount),
        "tbsp" => Some(amount * CUPS_PER_TBSP),
        "tsp" => Some(amount * CUPS_PER_TSP),
        "fl oz" => Some(amount * CUPS_PER_FL_OZ),
        "pint" => Some(amount * CUPS_PER_PINT),
        "quart" => Some(amount * CUPS_PER_QUART),
        "gallon" => Some(amount * CUPS_PER_GALLON),
        "ml" => Some(amount * CUPS_PER_ML),
        "l" => Some(amount * CUPS_PER_L),
        _ => None,
    }
}

/// Whether a canonical unit is a volume unit that densities can convert.
pub fn is_volume_unit(unit: Option<&str>) -> bool {
    unit.is_some_and(|unit| volume_to_cups(1.0, unit).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_to_cups() {
        assert_eq!(volume_to_cups(1.0, "cup"), Some(1.0));
        assert_eq!(volume_to_cups(16.0, "tbsp"), Some(1.0));
        assert_eq!(volume_to_cups(48.0, "tsp"), Some(1.0));
        assert_eq!(volume_to_cups(8.0, "fl oz"), Some(1.0));
        assert_eq!(volume_to_cups(0.5, "pint"), Some(1.0));
        assert_eq!(volume_to_cups(1.0, "oz"), None);
    }

    #[test]
    fn recognizes_volume_units() {
        assert!(is_volume_unit(Some("cup")));
        assert!(is_volume_unit(Some("tbsp")));
        assert!(is_volume_unit(Some("tsp")));
        assert!(!is_volume_unit(Some("oz")));
        assert!(!is_volume_unit(Some("lb")));
        assert!(!is_volume_unit(Some("g")));
        assert!(!is_volume_unit(None));
    }
}
