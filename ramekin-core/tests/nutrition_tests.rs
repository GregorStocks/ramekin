use ramekin_core::ingredient_parser::{Measurement, ParsedIngredient};
use ramekin_core::nutrition::estimate;

#[test]
fn scale_limits_match_both_clients() {
    #[derive(serde::Deserialize)]
    struct Vector {
        name: String,
        scale: f64,
        valid: bool,
    }
    let vectors: Vec<Vector> = serde_json::from_str(include_str!(
        "../../shared-test-vectors/recipe-scale-validation.json"
    ))
    .unwrap();
    for vector in vectors {
        assert_eq!(
            estimate(&[], None, vector.scale).is_ok(),
            vector.valid,
            "{}",
            vector.name
        );
    }
}

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
        ("1,5", "kg", 5805.0, 5805.0),
        (",5", "kg", 1935.0, 1935.0),
        ("0,125", "kg", 483.75, 483.75),
        ("1,5–2,5", "kg", 5805.0, 9675.0),
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
    for servings in [
        "4",
        "serves 4",
        "4 servings",
        " 4 SERVINGS ",
        "Servings: 4",
        "SERVINGS:4",
        "Serves: 4",
    ] {
        assert_eq!(
            estimate(std::slice::from_ref(&sugar), Some(servings), 2.0)
                .unwrap()
                .per_serving_calories
                .unwrap()
                .min,
            96.75
        );
    }
    for amount in ["1,000", "1,000.5", "1,2,3"] {
        let result = estimate(&[ingredient("granulated sugar", amount, "g")], None, 1.0).unwrap();
        assert!(result.known_calories.is_none());
    }
    let salt = estimate(&[ingredient("table salt", "1", "g")], None, 1.0).unwrap();
    assert_eq!(salt.known_calories.unwrap().min, 0.0);
    assert!(salt.unknown_ingredients.is_empty());
    assert!(estimate(&[], None, 1.0).unwrap().known_calories.is_none());
    for scale in [0.0, -1.0, f64::INFINITY, f64::NAN, 1e7] {
        assert!(estimate(&[], None, scale).is_err());
    }
}

#[test]
fn catalog_defaults_and_density_gaps() {
    let kcal = |item: &str, amount: &str, unit: &str| {
        let result = estimate(&[ingredient(item, amount, unit)], None, 1.0).unwrap();
        assert!(
            result.unknown_ingredients.is_empty(),
            "{item}: {:?}",
            result.unknown_ingredients
        );
        result.known_calories.unwrap().min
    };
    // Names nutrition used to refuse as ambiguous now share density's defaults.
    assert_eq!(
        kcal("sugar", "1", "cup"),
        kcal("granulated sugar", "1", "cup")
    );
    assert!(
        (kcal("flour", "1", "cup") - 455.0).abs() < 0.5,
        "125 g/cup x 364 kcal"
    );
    assert!((kcal("Salted Butter", "100", "g") - 717.0).abs() < 0.5);
    assert_eq!(kcal("kosher salt", "1", "tbsp"), 0.0);
    // Resolved by modifier stripping and plural fallback, like density.
    assert_eq!(
        kcal("melted butter", "1", "tbsp"),
        kcal("butter", "1", "tbsp")
    );
    // Pepper is known for calories but has no density, so only weights work.
    assert!(kcal("freshly ground black pepper", "10", "g") > 0.0);
    let by_volume = estimate(
        &[ingredient("freshly ground black pepper", "1", "tsp")],
        None,
        1.0,
    )
    .unwrap();
    assert_eq!(
        by_volume.unknown_ingredients[0].reason,
        "Missing density for this food"
    );
}

fn bare(item: &str) -> ParsedIngredient {
    ParsedIngredient {
        item: item.into(),
        measurements: vec![],
        note: Some("to taste".into()),
        raw: None,
        section: None,
    }
}

#[test]
fn negligible_lines_are_known_zero() {
    let known = |ingredient: ParsedIngredient| {
        let result = estimate(&[ingredient], None, 1.0).unwrap();
        assert!(
            result.unknown_ingredients.is_empty(),
            "{:?}",
            result.unknown_ingredients
        );
        result.known_calories.unwrap().max
    };
    // Zero-calorie foods, with or without a usable amount.
    assert_eq!(known(bare("kosher salt")), 0.0);
    assert_eq!(known(ingredient("salt", "1", "pinch")), 0.0);
    assert_eq!(known(bare("water")), 0.0);
    // Spices without a real amount.
    assert_eq!(known(bare("freshly ground black pepper")), 0.0);
    assert_eq!(known(ingredient("ground cumin", "1", "pinch")), 0.0);
    assert_eq!(known(ingredient("cayenne", "a few", "shakes")), 0.0);
    // Compounds whose every food is negligible on the line.
    assert_eq!(known(bare("salt and pepper")), 0.0);
    assert_eq!(
        known(bare("Kosher salt and freshly ground black pepper to taste")),
        0.0
    );
    // A real amount of a spice counts in full.
    assert!(
        (known(ingredient("ground cumin", "1", "cup")) - 360.0).abs() < 0.5,
        "96 g x 375 kcal"
    );
}

#[test]
fn compound_and_non_food_lines() {
    let result = estimate(
        &[
            ingredient("olive oil and balsamic vinegar", "2", "tbsp"),
            bare("And for the other side of the world:"),
            ingredient("parchment paper", "1", "sheet"),
            ingredient("granulated sugar", "100", "g"),
        ],
        None,
        1.0,
    )
    .unwrap();
    // One amount can't be split between two caloric foods.
    assert_eq!(result.unknown_ingredients.len(), 1);
    assert_eq!(
        result.unknown_ingredients[0].reason,
        "Several ingredients share one amount"
    );
    // The header and the parchment paper are skipped, not unknown.
    assert_eq!(result.known_calories.unwrap().max, 387.0);

    let equipment = estimate(&[bare("parchment paper and aluminum foil")], None, 1.0).unwrap();
    assert!(
        equipment.unknown_ingredients.is_empty(),
        "an all-product compound is skipped"
    );
    assert!(equipment.known_calories.is_none());

    let only_skipped = estimate(&[bare("to serve")], None, 1.0).unwrap();
    assert!(only_skipped.unknown_ingredients.is_empty());
    assert_eq!(only_skipped.summary, "No ingredients to estimate.");
}

#[test]
fn frying_oil_is_not_charged_in_full() {
    for (item, note) in [
        ("neutral oil, such as vegetable oil, for frying", None),
        ("vegetable oil", Some("for deep-frying")),
    ] {
        let mut line = ingredient(item, "2", "quart");
        line.note = note.map(str::to_string);
        let result = estimate(&[line], None, 1.0).unwrap();
        assert_eq!(
            result.unknown_ingredients[0].reason, "Frying oil: only part of it is absorbed",
            "{item}"
        );
    }
    // Only oils and fats are a frying medium; dredging flour stays in the dish.
    let mut flour = ingredient("all-purpose flour", "2", "cup");
    flour.note = Some("for frying".into());
    assert!(estimate(&[flour], None, 1.0)
        .unwrap()
        .unknown_ingredients
        .is_empty());
    // The measured part of "plus more for frying" is used in the recipe.
    let mut line = ingredient("vegetable oil", "2", "tbsp");
    line.note = Some("plus more for frying".into());
    assert!(estimate(&[line], None, 1.0)
        .unwrap()
        .unknown_ingredients
        .is_empty());
}

#[test]
fn a_spoonful_of_oil_for_frying_still_counts() {
    let mut line = ingredient("olive oil", "1", "tbsp");
    line.note = Some("for frying the meatballs".into());
    let result = estimate(&[line], None, 1.0).unwrap();
    assert!(
        result.unknown_ingredients.is_empty(),
        "{:?}",
        result.unknown_ingredients
    );
    assert!(result.known_calories.unwrap().max > 100.0);
}
