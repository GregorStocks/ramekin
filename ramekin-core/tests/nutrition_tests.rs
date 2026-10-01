use ramekin_core::ingredient_parser::{Measurement, ParsedIngredient};
use ramekin_core::nutrition::{estimate, Status, MAX_UNKNOWN_LINES};

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
        // USDA has no per-head weight for garlic, only per clove.
        ("garlic", "1", "head", "Unsupported quantity unit"),
        ("flour", "2", "", "Unsupported quantity unit"),
        ("canned chickpeas", "1", "can", "Unsupported quantity unit"),
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
        assert_eq!(result.status, Status::Insufficient);
        assert_eq!(
            result.headline,
            "Not enough ingredient data to estimate calories"
        );
        let partial = estimate(
            &[ingredient("granulated sugar", "100", "g"), unknown],
            Some("4"),
            1.0,
        )
        .unwrap();
        assert_eq!(partial.known_calories.unwrap().min, 387.0);
        assert_eq!(partial.unknown_ingredients[0].index, 1);
        assert_eq!(partial.status, Status::Partial);
        assert_eq!(partial.headline, "At least ~96 kcal per serving");
        assert_eq!(
            partial.secondary.as_deref(),
            Some("At least ~380 kcal for the whole recipe")
        );
        assert_eq!(partial.not_counted, [item]);
    }
}

#[test]
fn deterministic_matching_zero_calories_and_servings() {
    let sugar = ingredient("  GRANULATED   SUGAR ", "100", "grams");
    let first = estimate(std::slice::from_ref(&sugar), Some("4"), 1.0).unwrap();
    let second = estimate(std::slice::from_ref(&sugar), Some("4"), 1.0).unwrap();
    assert_eq!(first.database_version, second.database_version);
    assert_eq!(first.headline, second.headline);
    for servings in [
        "0",
        "-1",
        "6-4",
        "1 loaf",
        "makes 12 cookies",
        "Makes 24",
        "yield: 1 loaf",
        "NaN",
    ] {
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
        "Servings 4",
        "Yield: 4",
        "yield 4",
        "Makes 4 servings",
        "serves 4 people",
        "Yield: 4 portions",
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
    // Pepper is known for calories but has no density, so only weights work
    // (a spoonful or less is negligible; see the trace-food test).
    assert!(kcal("freshly ground black pepper", "10", "g") > 0.0);
    let by_volume = estimate(
        &[ingredient("freshly ground black pepper", "1/4", "cup")],
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

fn count(item: &str, amount: &str) -> ParsedIngredient {
    ParsedIngredient {
        measurements: vec![Measurement {
            amount: Some(amount.into()),
            unit: None,
        }],
        ..bare(item)
    }
}

#[test]
fn counted_ingredients_use_usda_piece_weights() {
    let kcal = |ingredient: ParsedIngredient| {
        let result = estimate(&[ingredient], None, 1.0).unwrap();
        assert!(
            result.unknown_ingredients.is_empty(),
            "{:?}",
            result.unknown_ingredients
        );
        result.known_calories.unwrap().min
    };
    let grams = |item: &str, grams: f64| kcal(ingredient(item, &grams.to_string(), "g"));
    for (counted, item, expected_grams) in [
        // A bare count of eggs means large eggs (a curated override).
        (count("eggs", "3"), "eggs", 150.0),
        (ingredient("eggs", "2", "large"), "eggs", 100.0),
        (ingredient("eggs", "1", "extra-large"), "eggs", 56.0),
        (ingredient("egg yolks", "2", "large"), "egg yolks", 34.0),
        (ingredient("garlic", "2", "cloves"), "garlic", 6.0),
        (ingredient("onion", "1", "medium"), "onion", 110.0),
        (count("onion", "1"), "onion", 110.0),
        // Sized pieces: "stalk" is the medium stalk, and a size finds the
        // food's one sized piece ("potato medium").
        (ingredient("celery", "1", "stalk"), "celery", 40.0),
        (ingredient("potatoes", "2", "medium"), "potatoes", 426.0),
        // A size the food has no portion for is its default piece.
        (ingredient("lemon", "1", "large"), "lemon", 58.0),
        (ingredient("butter", "1 1/2", "sticks"), "butter", 169.5),
        (count("jalapeño pepper", "2"), "jalapeño pepper", 28.0),
        (count("basil leaves", "10"), "basil leaves", 5.0),
    ] {
        let expected = grams(item, expected_grams);
        assert!(
            (kcal(counted.clone()) - expected).abs() < 1e-9,
            "{counted:?}: expected {expected_grams} g"
        );
    }
}

#[test]
fn packages_carry_their_own_weight_and_fill_words_are_ignored() {
    let kcal = |item: &str, amount: &str, unit: &str| {
        let result = estimate(&[ingredient(item, amount, unit)], None, 1.0).unwrap();
        assert!(
            result.unknown_ingredients.is_empty(),
            "{unit}: {:?}",
            result.unknown_ingredients
        );
        result.known_calories.unwrap().min
    };
    let can = kcal("chickpeas", "425.242846875", "g");
    for unit in ["15-ounce can", "(15-ounce) can", "15 oz can", "15-oz. cans"] {
        assert!((kcal("chickpeas", "1", unit) - can).abs() < 1e-9, "{unit}");
    }
    // The parser moves a parenthesized weight into the note:
    // "2 (15-ounce) cans chickpeas" is unit "can", note "15-ounce, drained".
    for note in ["15-ounce, drained", "15 oz", "15-ounce or 425-gram"] {
        let noted = ParsedIngredient {
            note: Some(note.into()),
            ..ingredient("chickpeas", "2", "cans")
        };
        let result = estimate(&[noted], None, 1.0).unwrap();
        assert!(
            (result.known_calories.unwrap().min - 2.0 * can).abs() < 1e-9,
            "{note}"
        );
    }
    let ounces = |oz: f64| kcal("chickpeas", &(oz * 28.349523125).to_string(), "g");
    let noted = ParsedIngredient {
        note: Some("14 1/2-ounce".into()),
        ..ingredient("chickpeas", "1", "can")
    };
    let result = estimate(&[noted], None, 1.0).unwrap();
    assert!((result.known_calories.unwrap().min - ounces(14.5)).abs() < 1e-9);
    let ranged = estimate(
        &[ingredient("chickpeas", "2", "12- to 18-ounce packages")],
        None,
        1.0,
    )
    .unwrap()
    .known_calories
    .unwrap();
    assert!((ranged.min - ounces(24.0)).abs() < 1e-9);
    assert!((ranged.max - ounces(36.0)).abs() < 1e-9);
    for note in ["drained", "about 15-ounce", "15-ounce-ish"] {
        let noted = ParsedIngredient {
            note: Some(note.into()),
            ..ingredient("chickpeas", "1", "can")
        };
        let result = estimate(&[noted], None, 1.0).unwrap();
        assert!(result.known_calories.is_none(), "{note}");
    }
    assert!((kcal("chickpeas", "1", "425-gram can") - kcal("chickpeas", "425", "g")).abs() < 1e-9);
    for unit in [
        "heaped tsp",
        "heaping tsp",
        "scant tsp",
        "slightly heaped tsp",
    ] {
        assert_eq!(
            kcal("granulated sugar", "1", unit),
            kcal("granulated sugar", "1", "tsp")
        );
    }
}

#[test]
fn counted_trace_foods_without_a_piece_weight_are_negligible() {
    let known = |ingredient: ParsedIngredient| {
        let result = estimate(&[ingredient], None, 1.0).unwrap();
        assert!(
            result.unknown_ingredients.is_empty(),
            "{:?}",
            result.unknown_ingredients
        );
        result.known_calories.unwrap().max
    };
    assert_eq!(known(count("bay leaves", "2")), 0.0);
    assert_eq!(known(ingredient("bay leaves", "2", "leaves")), 0.0);
    assert_eq!(known(ingredient("cinnamon", "1", "stick")), 0.0);
    assert_eq!(known(count("bay leaves", "6-8")), 0.0);
    // Past a handful, including after scaling, the calories are unknown
    // rather than zero.
    let doubled = estimate(&[count("bay leaves", "6-8")], None, 2.0).unwrap();
    assert!(doubled.known_calories.is_none());
    for amount in ["100", "8-12"] {
        let many = estimate(&[count("bay leaves", amount)], None, 1.0).unwrap();
        assert!(many.known_calories.is_none(), "{amount}");
    }
    assert_eq!(known(ingredient("thyme", "4", "sprig")), 0.0);
    // A measured amount of a fresh herb still counts.
    assert!(known(ingredient("basil", "10", "g")) > 0.0);
    // Only small pieces of trace foods: garlic heads are not negligible.
    let head = estimate(&[ingredient("garlic", "1", "head")], None, 1.0).unwrap();
    assert!(head.known_calories.is_none());
}

#[test]
fn a_spoonful_of_an_unweighable_trace_food_is_negligible() {
    let calories = |item: &str, amount: &str, unit: &str, scale: f64| {
        let result = estimate(&[ingredient(item, amount, unit)], None, scale).unwrap();
        result.known_calories.map(|range| range.max)
    };
    // Black pepper has no density (grind size varies too much).
    for unit in ["tsp", "teaspoons", "tbsp", "heaping tsp", "ml"] {
        assert_eq!(
            calories("freshly ground black pepper", "1", unit, 1.0),
            Some(0.0),
            "{unit}"
        );
    }
    assert_eq!(calories("black pepper", "1/2-1", "tbsp", 1.0), Some(0.0));
    // Compound spoons are summed: 1 1/2 tsp is under a tablespoon, 2 tbsp isn't.
    let compound = |amount: &str| {
        let line = ParsedIngredient {
            measurements: vec![Measurement {
                amount: Some(amount.into()),
                unit: None,
            }],
            ..ingredient("black pepper", "1", "tsp")
        };
        estimate(&[line], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.max)
    };
    assert_eq!(compound("1 teaspoon plus 1/2 teaspoon"), Some(0.0));
    assert_eq!(compound("1 tbsp + 1 tbsp"), None);
    // Past a tablespoon, including after scaling, it's unknown, not zero.
    assert_eq!(calories("black pepper", "2", "tbsp", 1.0), None);
    assert_eq!(calories("black pepper", "1", "tbsp", 2.0), None);
    assert_eq!(calories("black pepper", "1", "cup", 1.0), None);
    // A trace food with a density is computed normally.
    assert!(calories("ground cumin", "1", "tbsp", 1.0).unwrap() > 0.0);
    // Only trace foods: a spoonful of anything else still needs a density.
    let result = estimate(&[ingredient("capers", "1", "tbsp")], None, 1.0).unwrap();
    assert!(result.known_calories.is_none());
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
    assert_eq!(only_skipped.status, Status::Empty);
    assert_eq!(only_skipped.headline, "No ingredients to estimate");
    assert_eq!(only_skipped.lines[0].text, "Not a food");
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

#[test]
fn status_follows_the_number_of_uncounted_ingredients() {
    let sugar = || ingredient("granulated sugar", "100", "g");
    let unknown = |n: usize| {
        (0..n)
            .map(|i| ingredient(&format!("moon dust {i}"), "1", "cup"))
            .collect::<Vec<_>>()
    };
    let complete = estimate(&[sugar()], None, 1.0).unwrap();
    assert_eq!(complete.status, Status::Complete);
    assert_eq!(complete.headline, "~390 kcal for the whole recipe");
    assert_eq!(complete.secondary, None);
    assert!(complete.not_counted.is_empty());

    let mut lines = vec![sugar()];
    lines.extend(unknown(MAX_UNKNOWN_LINES));
    let partial = estimate(&lines, None, 1.0).unwrap();
    assert_eq!(partial.status, Status::Partial);
    assert_eq!(partial.headline, "At least ~380 kcal for the whole recipe");
    assert_eq!(partial.not_counted.len(), MAX_UNKNOWN_LINES);

    lines.extend(unknown(1));
    let insufficient = estimate(&lines, None, 1.0).unwrap();
    assert_eq!(insufficient.status, Status::Insufficient);
    assert_eq!(
        insufficient.secondary.as_deref(),
        Some(format!("{} ingredients couldn't be counted.", MAX_UNKNOWN_LINES + 1).as_str())
    );
    assert!(insufficient.not_counted.is_empty());

    // Negligible and non-food lines are never uncounted.
    let seasoned = estimate(
        &[
            sugar(),
            bare("salt and pepper"),
            bare("to serve"),
            bare("Sauce:"),
        ],
        None,
        1.0,
    )
    .unwrap();
    assert_eq!(seasoned.status, Status::Complete);
    let texts: Vec<_> = seasoned.lines.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(
        texts,
        ["~390 kcal", "Negligible", "Not a food", "Not a food"]
    );

    // A real food without a usable amount stays uncounted.
    let greased = estimate(&[sugar(), bare("butter")], None, 1.0).unwrap();
    assert_eq!(greased.status, Status::Partial);
    assert_eq!(greased.lines[1].text, "Amount unclear");

    // Salt beside an unknown main ingredient is no lower bound at all.
    let only_salt = estimate(
        &[bare("kosher salt"), ingredient("moon dust", "1", "cup")],
        None,
        1.0,
    )
    .unwrap();
    assert_eq!(only_salt.status, Status::Insufficient);
    let from_zero = estimate(
        &[
            ingredient("granulated sugar", "0-100", "g"),
            ingredient("moon dust", "1", "cup"),
        ],
        None,
        1.0,
    )
    .unwrap();
    assert_eq!(from_zero.status, Status::Insufficient);
    let pinch = estimate(
        &[
            ingredient("granulated sugar", "0.1", "g"),
            ingredient("moon dust", "1", "cup"),
        ],
        None,
        1.0,
    )
    .unwrap();
    assert_eq!(pinch.status, Status::Insufficient);

    let empty = estimate(&[], Some("4"), 1.0).unwrap();
    assert_eq!(empty.status, Status::Empty);
}

#[test]
fn headlines_lead_with_per_serving_and_format_numbers() {
    let flour = ingredient("flour", "2", "cups");
    let per_serving = estimate(std::slice::from_ref(&flour), Some("4"), 1.0).unwrap();
    assert_eq!(per_serving.headline, "~230 kcal per serving");
    assert_eq!(
        per_serving.secondary.as_deref(),
        Some("~910 kcal for the whole recipe")
    );
    // Thousands separators, lines scaled with the recipe.
    let tripled = estimate(std::slice::from_ref(&flour), None, 3.0).unwrap();
    assert_eq!(tripled.headline, "~2,730 kcal for the whole recipe");
    assert_eq!(tripled.lines[0].text, "~2,730 kcal");
    let ranged = estimate(
        &[ingredient("granulated sugar", "100-200", "g")],
        Some("serves 4"),
        1.0,
    )
    .unwrap();
    assert_eq!(ranged.headline, "~96–200 kcal per serving");
    let tiny = estimate(&[ingredient("granulated sugar", "0.1", "g")], None, 1.0).unwrap();
    assert_eq!(tiny.headline, "<1 kcal for the whole recipe");
    let tiny_range =
        estimate(&[ingredient("granulated sugar", "0.1-0.2", "g")], None, 1.0).unwrap();
    assert_eq!(tiny_range.headline, "<1 kcal for the whole recipe");
    let from_tiny = estimate(&[ingredient("granulated sugar", "0.1-2", "g")], None, 1.0).unwrap();
    assert_eq!(from_tiny.headline, "~0–8 kcal for the whole recipe");
}

#[test]
fn serving_ranges_give_a_per_serving_range() {
    let sugar = ingredient("granulated sugar", "240", "g");
    for servings in ["4 to 6 servings", "Serves 4 to 6", "Servings: 4-6", "4–6"] {
        let result = estimate(std::slice::from_ref(&sugar), Some(servings), 1.0).unwrap();
        let per_serving = result.per_serving_calories.unwrap();
        // 928.8 kcal over 6 servings to 928.8 over 4.
        assert!((per_serving.min - 154.8).abs() < 1e-9, "{servings}");
        assert!((per_serving.max - 232.2).abs() < 1e-9, "{servings}");
        assert_eq!(result.headline, "~150–240 kcal per serving", "{servings}");
    }
}

#[test]
fn secondary_source_foods_count() {
    // FNDDS guacamole (155 kcal/100 g) and cited garam masala (300 kcal/100 g).
    let total = |item: &str, amount: &str, unit: &str| {
        estimate(&[ingredient(item, amount, unit)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.max)
    };
    assert_eq!(total("guacamole", "100", "g"), Some(155.0));
    assert_eq!(total("garam masala", "100", "g"), Some(300.0));
    // Its cited density weighs volumes: 1 tsp is 160/48 g, about 10 kcal.
    assert!((total("garam masala", "1", "tsp").unwrap() - 10.0).abs() < 1e-9);
    assert!((total("garam masala", "1/4", "cup").unwrap() - 120.0).abs() < 1e-9);
}

#[test]
fn trace_only_spices_are_negligible_in_small_amounts() {
    // Sumac has no citable calories: a trace entry, negligible for a pinch,
    // no amount, or up to a tablespoon, and unknown beyond that.
    let calories = |line: ParsedIngredient| {
        estimate(&[line], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.max)
    };
    assert_eq!(calories(ingredient("sumac", "1", "pinch")), Some(0.0));
    assert_eq!(calories(ingredient("sumac", "2", "tsp")), Some(0.0));
    assert_eq!(calories(bare("ground sumac")), Some(0.0));
    assert_eq!(calories(ingredient("sumac", "1/4", "cup")), None);
    let result = estimate(&[ingredient("sumac", "100", "g")], None, 1.0).unwrap();
    assert_eq!(
        result.unknown_ingredients[0].reason,
        "No supported nutrition match"
    );
    // Fresh herbs listed by the leaf are a trace too.
    assert_eq!(
        calories(ingredient("fresh sage leaves", "6", "")),
        Some(0.0)
    );
}

#[test]
fn za_atar_spellings_share_one_trace_entry() {
    for name in ["za’atar", "za'atar", "zaatar"] {
        let result = estimate(&[ingredient(name, "1", "tsp")], None, 1.0).unwrap();
        assert_eq!(
            result.known_calories.map(|range| range.max),
            Some(0.0),
            "{name}"
        );
    }
}

#[test]
fn mustard_seeds_use_usda_calories_without_a_volume_weight() {
    let calories = |amount: &str, unit: &str| {
        estimate(&[ingredient("mustard seeds", amount, unit)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.max)
    };
    // USDA ground mustard seed: 508 kcal/100 g.
    assert!((calories("10", "g").unwrap() - 50.8).abs() < 1e-9);
    // Whole seeds have no volume weight: a spoonful is a trace, more is unknown.
    assert_eq!(calories("1", "tbsp"), Some(0.0));
    assert_eq!(calories("1/4", "cup"), None);
}
