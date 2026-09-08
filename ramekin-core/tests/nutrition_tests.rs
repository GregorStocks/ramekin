use ramekin_core::ingredient_parser::{Measurement, ParsedIngredient};
use ramekin_core::nutrition::estimate;

fn ingredient(item: &str, amount: &str, unit: &str) -> ParsedIngredient {
    ParsedIngredient {
        item: item.into(),
        measurements: vec![Measurement {
            amount: Some(amount.into()),
            unit: Some(unit.into()),
        }],
        note: None,
        raw: None,
        section: None,
    }
}

#[test]
fn exact_ranges_units_and_scaling() {
    for (amount, unit, min, max) in [
        ("100", "g", 387.0, 387.0),
        ("0.1", "kg", 387.0, 387.0),
        ("100000", "mg", 387.0, 387.0),
        ("1", "oz", 109.71265449375, 109.71265449375),
        ("1", "lb", 1755.4024719, 1755.4024719),
        ("100–200", "g", 387.0, 774.0),
        ("100 to 200", "g", 387.0, 774.0),
        ("100-200", "g", 387.0, 774.0),
        ("1/2", "kg", 1935.0, 1935.0),
        ("1½", "kg", 5805.0, 5805.0),
        ("1-1/2", "kg", 5805.0, 5805.0),
        ("1-1/2-2", "kg", 5805.0, 7740.0),
        ("1/2-3/4", "kg", 1935.0, 2902.5),
    ] {
        for scale in [f64::from_bits(1), 0.5, 1.0, 2.0] {
            let result = estimate(
                &[ingredient("granulated sugar", amount, unit)],
                Some("2"),
                scale,
            )
            .unwrap();
            let total = result.known_calories.unwrap();
            assert!((total.min - min * scale).abs() < 1e-6, "{amount} {unit}");
            assert!((total.max - max * scale).abs() < 1e-6);
            assert!((result.per_serving_calories.unwrap().min - min / 2.0).abs() < 1e-6);
            assert!(result.unknown_ingredients.is_empty());
        }
    }
}

#[test]
fn compounds_sum_every_supported_segment_and_keep_ranges() {
    for (amount, unit, min, max) in [
        ("1 tablespoon plus 1 teaspoon", "", 64.5, 64.5),
        ("1/2 cup plus 1–2 tbsp plus 1 tsp", "", 451.5, 499.875),
        ("1/4 plus 1/8", "cup", 290.25, 290.25),
        ("1 g + 1 kg", "", 3873.87, 3873.87),
    ] {
        let result = estimate(&[ingredient("granulated sugar", amount, unit)], None, 2.0).unwrap();
        let range = result.known_calories.unwrap();
        assert!((range.min - min * 2.0).abs() < 1e-6, "{amount}");
        assert!((range.max - max * 2.0).abs() < 1e-6, "{amount}");
    }
    for amount in [
        "1 tablespoon plus more to taste",
        "1 cup plus 1/0 tbsp",
        "1 cup plus 1 pinch",
        "1 cup plus ",
    ] {
        let result = estimate(&[ingredient("granulated sugar", amount, "")], None, 1.0).unwrap();
        assert!(result.known_calories.is_none(), "{amount}");
        assert_eq!(result.unknown_ingredients.len(), 1);
    }
}

#[test]
fn volume_conversions_and_alternatives_are_not_added() {
    for (amount, unit) in [
        ("1", "cup"),
        ("16", "tbsp"),
        ("48", "tsp"),
        ("8", "fl oz"),
        ("236.588", "ml"),
        ("0.236588", "l"),
    ] {
        let result = estimate(&[ingredient("granulated sugar", amount, unit)], None, 1.0).unwrap();
        let total = result.known_calories.unwrap();
        assert!((total.min - 774.0).abs() < 1e-9);
        assert!((total.max - 774.0).abs() < 1e-9);
    }
    let mut sugar = ingredient("granulated sugar", "1", "cup");
    sugar.measurements.push(Measurement {
        amount: Some("200".into()),
        unit: Some("g".into()),
    });
    assert_eq!(
        estimate(&[sugar], None, 1.0)
            .unwrap()
            .known_calories
            .unwrap()
            .min,
        774.0
    );
    let mut eggs = ingredient("eggs", "2", "large");
    eggs.measurements.push(Measurement {
        amount: Some("100".into()),
        unit: Some("g".into()),
    });
    assert_eq!(
        estimate(&[eggs], None, 1.0)
            .unwrap()
            .known_calories
            .unwrap()
            .min,
        143.0
    );
}

#[test]
fn unknowns_are_explicit_and_never_zero() {
    for (item, amount, unit, reason) in [
        ("yogurt", "100", "g", "Ambiguous ingredient"),
        ("moon dust", "100", "g", "No supported nutrition match"),
        (
            "granulated sugar",
            "to taste",
            "g",
            "Unsupported or missing quantity",
        ),
        (
            "granulated sugar",
            "200-100",
            "g",
            "Unsupported or missing quantity",
        ),
        (
            "granulated sugar",
            "-100",
            "g",
            "Unsupported or missing quantity",
        ),
        (
            "granulated sugar",
            "1/0",
            "g",
            "Unsupported or missing quantity",
        ),
        (
            "granulated sugar",
            "NaN",
            "g",
            "Unsupported or missing quantity",
        ),
        ("eggs", "2", "", "Unsupported quantity unit"),
        (
            "waffles, buttermilk, frozen, ready-to-heat",
            "1",
            "cup",
            "Missing density for this food",
        ),
    ] {
        let unknown = ingredient(item, amount, unit);
        let result = estimate(std::slice::from_ref(&unknown), Some("4"), 1.0).unwrap();
        assert!(result.known_calories.is_none(), "{item} {amount} {unit}");
        assert!(result.per_serving_calories.is_none());
        assert_eq!(result.unknown_ingredients[0].reason, reason);
        assert!(result.summary.contains("unknown"));
        let partial = estimate(
            &[ingredient("granulated sugar", "100", "g"), unknown],
            Some("4"),
            1.0,
        )
        .unwrap();
        assert_eq!(partial.known_calories.unwrap().min, 387.0);
        assert_eq!(partial.unknown_ingredients[0].index, 1);
        assert!(partial.summary.contains("partial whole-recipe subtotal"));
        assert!(partial
            .per_serving_summary
            .unwrap()
            .contains("partial subtotal"));
    }
}

#[test]
fn deterministic_matching_zero_calories_and_servings() {
    let sugar = ingredient("  GRANULATED   SUGAR ", "100", "grams");
    let first = estimate(std::slice::from_ref(&sugar), Some("4"), 1.0).unwrap();
    let second = estimate(std::slice::from_ref(&sugar), Some("4"), 1.0).unwrap();
    assert_eq!(first.database_version, second.database_version);
    assert_eq!(first.summary, second.summary);
    for servings in ["0", "-1", "4–6", "1 loaf", "4–6 servings", "NaN"] {
        assert!(estimate(std::slice::from_ref(&sugar), Some(servings), 1.0)
            .unwrap()
            .per_serving_calories
            .is_none());
    }
    for servings in ["4", "serves 4", "4 servings", " 4 SERVINGS "] {
        assert_eq!(
            estimate(std::slice::from_ref(&sugar), Some(servings), 2.0)
                .unwrap()
                .per_serving_calories
                .unwrap()
                .min,
            96.75
        );
    }
    let salt = estimate(&[ingredient("table salt", "1", "g")], None, 1.0).unwrap();
    assert_eq!(salt.known_calories.unwrap().min, 0.0);
    assert!(salt.unknown_ingredients.is_empty());
    assert!(estimate(&[], None, 1.0).unwrap().known_calories.is_none());
    for scale in [0.0, -1.0, f64::INFINITY, f64::NAN, 1e7] {
        assert!(estimate(&[], None, scale).is_err());
    }
}
