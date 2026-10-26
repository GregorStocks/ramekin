use ramekin_core::catalog;
use ramekin_core::ingredient_parser::{parse_ingredient, Measurement, ParsedIngredient};
use ramekin_core::nutrition::{estimate, Status, WeightKey, MAX_UNKNOWN_LINES};

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
        // A pinch is 1/16 tsp, 1/768 cup.
        ("1 cup plus 1 pinch", "", 775.0078125, 775.0078125),
    ] {
        let result = estimate(&[ingredient("granulated sugar", amount, unit)], None, 2.0).unwrap();
        let range = result.known_calories.unwrap();
        assert!((range.min - min * 2.0).abs() < 1e-6, "{amount}");
        assert!((range.max - max * 2.0).abs() < 1e-6, "{amount}");
    }
    for amount in [
        "1 tablespoon plus more to taste",
        "1 cup plus 1/0 tbsp",
        "1 cup plus 1 handful",
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
        // Kale is weighed by the bunch, but a bare count means nothing.
        ("kale", "2", "", "Unsupported quantity unit"),
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
    assert_eq!(known(ingredient("saffron", "1", "large pinch")), 0.0);
    assert_eq!(known(ingredient("kosher salt", "1", "small pinch")), 0.0);
    assert_eq!(known(ingredient("saffron", "1", "extra-large pinch")), 0.0);
    // A quality word after a package weight leaves the weight leading the note.
    let can = |raw: &str| {
        let parsed = ramekin_core::ingredient_parser::parse_ingredient(raw);
        estimate(&[parsed], None, 1.0)
            .unwrap()
            .known_calories
            .unwrap()
            .min
    };
    assert_eq!(
        can("1 (28-ounce) can good quality whole tomatoes"),
        can("1 (28-ounce) can whole tomatoes")
    );
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
        (parse_ingredient("3 extra large eggs"), "eggs", 168.0),
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
    let noted = |note: &str| {
        let line = ParsedIngredient {
            note: Some(note.into()),
            ..ingredient("chickpeas", "1", "can")
        };
        estimate(&[line], None, 1.0).unwrap().known_calories
    };
    // A note with no weight leaves the standard can (15.5 oz)...
    let standard = kcal("chickpeas", "439", "g");
    assert!((noted("drained").unwrap().max - standard).abs() < 1e-9);
    // ...but a stated weight that can't be read is an unclear amount.
    assert!(noted("about 15-ounce").is_none());
    assert!((kcal("chickpeas", "1", "425-gram can") - kcal("chickpeas", "425", "g")).abs() < 1e-9);
    for unit in [
        "heaped tsp",
        "heaping tsp",
        "scant tsp",
        "slightly heaped tsp",
        "packed tsp",
        "firmly packed tsp",
    ] {
        assert_eq!(
            kcal("granulated sugar", "1", unit),
            kcal("granulated sugar", "1", "tsp")
        );
    }
}

#[test]
fn tiny_measures_of_non_trace_foods_use_the_density() {
    let kcal = |item: &str, amount: &str, unit: &str| {
        let result = estimate(&[ingredient(item, amount, unit)], None, 1.0).unwrap();
        assert!(
            result.unknown_ingredients.is_empty(),
            "{item} {unit}: {:?}",
            result.unknown_ingredients
        );
        result.known_calories.unwrap().min
    };
    let tsp = kcal("granulated sugar", "1", "tsp");
    for (unit, per_tsp) in [
        ("dash", 1.0 / 8.0),
        ("pinch", 1.0 / 16.0),
        ("drop", 1.0 / 96.0),
    ] {
        assert!(
            (kcal("granulated sugar", "2", unit) - 2.0 * per_tsp * tsp).abs() < 1e-9,
            "{unit}"
        );
    }
    assert!(kcal("worcestershire sauce", "1", "dash") > 0.0);
    assert!(kcal("toasted sesame oil", "1", "dash") > 0.0);
    // A trace spice is still negligible.
    assert_eq!(kcal("ground cumin", "1", "dash"), 0.0);
    // A packed cup weighs as a plain one; brown sugar's density is packed.
    assert_eq!(
        kcal("brown sugar", "1", "packed cup"),
        kcal("brown sugar", "1", "cup")
    );
    // The same, from raw recipe text through the parser.
    let parsed = |raw: &str| {
        let result = estimate(&[parse_ingredient(raw)], None, 1.0).unwrap();
        assert!(
            result.unknown_ingredients.is_empty(),
            "{raw}: {:?}",
            result.unknown_ingredients
        );
        result.known_calories.unwrap().min
    };
    assert!(parsed("2 dashes Worcestershire sauce") > 0.0);
    assert!(parsed("3 drops Tabasco sauce") > 0.0);
    for raw in [
        "1 packed cup brown sugar",
        "1 firmly packed cup brown sugar",
        "1 lightly packed cup brown sugar",
    ] {
        assert_eq!(parsed(raw), kcal("brown sugar", "1", "cup"), "{raw}");
    }
}

#[test]
fn informal_measures_weigh_by_convention() {
    let kcal = |raw: &str| {
        let result = estimate(&[parse_ingredient(raw)], None, 1.0).unwrap();
        assert!(
            result.unknown_ingredients.is_empty(),
            "{raw}: {:?}",
            result.unknown_ingredients
        );
        result.known_calories.unwrap()
    };
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    // A sized handful uses the food's handful weight.
    for (sized, plain) in [
        ("1 large handful cilantro", "1 handful cilantro"),
        ("1 small handful mint leaves", "1 handful mint leaves"),
        ("2 big handfuls of chopped kale", "2 handfuls chopped kale"),
    ] {
        assert_eq!(kcal(sized), kcal(plain), "{sized}");
        assert!(kcal(sized).min > 0.0, "{sized}");
    }
    // A glug or a knob is 1 to 2 tbsp.
    let oil = kcal("1 tablespoon olive oil").min;
    let glugs = kcal("2 glugs of olive oil");
    assert!(close(glugs.min, 2.0 * oil) && close(glugs.max, 4.0 * oil));
    let butter = kcal("1 tablespoon butter").min;
    let knobs = kcal("1-2 good knobs butter");
    assert!(close(knobs.min, butter) && close(knobs.max, 4.0 * butter));
    // A shot of espresso is 1 fl oz; a scoop of ice cream 1/2 cup.
    assert!(close(
        kcal("2 shots espresso").min,
        kcal("2 fl oz espresso").min
    ));
    assert!(close(
        kcal("2 scoops vanilla ice cream").min,
        kcal("1 cup vanilla ice cream").min
    ));
    // An ice-cream scoop is never a question for the model.
    let ice_cream = estimate(&[parse_ingredient("2 scoops vanilla ice cream")], None, 1.0).unwrap();
    assert!(ice_cream.weight_gaps.is_empty());
    // A scoop of powder depends on the brand's scoop, so it's estimated per
    // food (a whey scoop is ~30 g by label convention).
    let powder = estimate(&[parse_ingredient("1 scoop protein powder")], None, 1.0).unwrap();
    assert!(close(
        powder.known_calories.unwrap().min,
        kcal("30 g protein powder").min
    ));
    assert!(
        powder.lines[0].text.ends_with("(estimated weight)"),
        "{}",
        powder.lines[0].text
    );
}

#[test]
fn inch_pieces_scale_the_one_inch_piece() {
    let kcal = |unit: &str| {
        let result = estimate(&[ingredient("ginger", "1", unit)], None, 1.0).unwrap();
        assert!(
            result.unknown_ingredients.is_empty(),
            "{unit}: {:?}",
            result.unknown_ingredients
        );
        result.known_calories.unwrap()
    };
    let inch = kcal("1-inch piece").min;
    for (unit, inches) in [
        ("3-inch piece", 3.0),
        ("3-inch knobs", 3.0),
        ("3/4-inch piece", 0.75),
        ("1 1/2-inch chunk", 1.5),
    ] {
        assert!((kcal(unit).min - inches * inch).abs() < 1e-9, "{unit}");
    }
    let range = kcal("3-4 inch chunk");
    assert!((range.min - 3.0 * inch).abs() < 1e-9);
    assert!((range.max - 4.0 * inch).abs() < 1e-9);
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
    // A size before the piece ("1 small bay leaf") counts the same pieces.
    assert_eq!(known(ingredient("bay leaf", "1", "small")), 0.0);
    assert_eq!(known(ingredient("cinnamon sticks", "2", "small")), 0.0);
    assert_eq!(known(ingredient("sprigs fresh thyme", "3", "large")), 0.0);
    assert_eq!(known(ingredient("bay leaves", "2", "extra-large")), 0.0);
    let many_large = estimate(&[ingredient("bay leaves", "20", "large")], None, 1.0).unwrap();
    assert!(many_large.known_calories.is_none());
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
    // Only small pieces of trace foods: a garlic head is counted (24 g).
    let head = estimate(&[ingredient("garlic", "1", "head")], None, 1.0).unwrap();
    assert!(head.known_calories.unwrap().min > 0.0);
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
    let result = estimate(
        &[ingredient(
            "waffles, buttermilk, frozen, ready-to-heat",
            "1",
            "tbsp",
        )],
        None,
        1.0,
    )
    .unwrap();
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
fn an_amount_of_another_recipe_says_so() {
    for raw in [
        "1 recipe guacamole",
        "1/2 recipe the best basic guacamole",
        "1 batch roasted tomato salsa or your favorite salsa, for serving",
        "1/2 batch Rice Krispies Treats",
        // Pizza dough is weighed by the ball, but a recipe of it isn't one.
        "1 recipe pizza dough",
        // With no amount left, the batter isn't negligible either.
        "1 recipe Yorkshire pudding batter, see above",
    ] {
        let result = estimate(&[parse_ingredient(raw)], None, 1.0).unwrap();
        assert_eq!(
            result.unknown_ingredients[0].reason, "Another recipe",
            "{raw}"
        );
        assert!(
            result.lines[0].text.starts_with("Refers to another recipe"),
            "{raw}: {}",
            result.lines[0].text
        );
    }
    // Another measurement on the line still counts it.
    let mut batch = parse_ingredient("1 batch guacamole");
    batch.measurements.push(Measurement {
        amount: Some("200".into()),
        unit: Some("g".into()),
    });
    assert_eq!(
        estimate(&[batch], None, 1.0).unwrap().known_calories,
        estimate(&[parse_ingredient("200 g guacamole")], None, 1.0)
            .unwrap()
            .known_calories
    );
    // So does a weight in the note.
    let mut noted = parse_ingredient("1 batch guacamole");
    noted.note = Some("about 200 g".into());
    assert_eq!(
        estimate(&[noted], None, 1.0).unwrap().known_calories,
        estimate(&[parse_ingredient("200 g guacamole")], None, 1.0)
            .unwrap()
            .known_calories
    );
    // "recipe" deeper in the name is just a word.
    let result = estimate(&[parse_ingredient("1 cup recipe-ready broth")], None, 1.0).unwrap();
    assert!(result
        .unknown_ingredients
        .iter()
        .all(|unknown| unknown.reason != "Another recipe"));
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

    // A real food given no amount is left out and listed, but doesn't make
    // the estimate partial: there's nothing to count.
    let greased = estimate(&[sugar(), bare("butter")], None, 1.0).unwrap();
    assert_eq!(greased.status, Status::Complete);
    assert_eq!(greased.headline, "~390 kcal for the whole recipe");
    assert_eq!(greased.lines[1].text, "No amount given");
    assert_eq!(greased.not_counted, ["butter"]);
    assert!(greased.unknown_ingredients.is_empty());
    assert_eq!(greased.no_amount[0].reason, "Missing quantity");
    // An unclear amount is still unknown.
    let handful = estimate(&[sugar(), ingredient("butter", "a knob", "")], None, 1.0).unwrap();
    assert_eq!(handful.status, Status::Partial);
    assert_eq!(handful.lines[1].text, "Amount unclear");
    // Partial estimates list both kinds, in recipe order.
    let both = estimate(
        &[
            bare("olive oil"),
            sugar(),
            ingredient("moon dust", "1", "cup"),
        ],
        None,
        1.0,
    )
    .unwrap();
    assert_eq!(both.status, Status::Partial);
    assert_eq!(both.not_counted, ["olive oil", "moon dust"]);
    // Nothing counted but lines with no amount: nothing to show.
    let nothing = estimate(&[bare("olive oil"), bare("butter")], None, 1.0).unwrap();
    assert_eq!(nothing.status, Status::Insufficient);
    assert_eq!(
        nothing.secondary.as_deref(),
        Some("2 ingredients couldn't be counted.")
    );
    // Salt is no figure to show either.
    let salted = estimate(&[bare("kosher salt"), bare("olive oil")], None, 1.0).unwrap();
    assert_eq!(salted.status, Status::Insufficient);

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

#[test]
fn whole_spices_use_their_ground_form_calories() {
    let calories = |item: &str, amount: &str, unit: &str| {
        estimate(&[ingredient(item, amount, unit)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.max)
    };
    // USDA ground allspice: 263 kcal/100 g; whole berries have no volume weight.
    assert!((calories("allspice berries", "10", "g").unwrap() - 26.3).abs() < 1e-9);
    assert_eq!(calories("allspice berries", "1/4", "cup"), None);
    // Recipes use "Japanese chili powder" for shichimi togarashi, a blend
    // with no citable calories: a spoonful is a trace, more is unknown.
    assert_eq!(calories("japanese chili powder", "1", "tsp"), Some(0.0));
    assert_eq!(calories("japanese chili powder", "1/4", "cup"), None);
}

#[test]
fn fresh_herb_and_star_anise_forms_reach_their_trace_entries() {
    for item in [
        "chopped fresh sage",
        "minced fresh sage",
        "fresh sage",
        "chopped fresh tarragon",
        "fresh tarragon",
        "star anise pods",
        "star anise pod",
        "peach bitters",
        "peychaud's bitters",
        "aromatic bitters",
        "shichimi",
        "aleppo pepper flakes",
    ] {
        let small = estimate(&[ingredient(item, "1", "tbsp")], None, 1.0).unwrap();
        assert_eq!(
            small.known_calories.map(|range| range.max),
            Some(0.0),
            "{item}"
        );
        // Not anise seed or dried sage: a weighed amount stays unknown.
        let weighed = estimate(&[ingredient(item, "10", "g")], None, 1.0).unwrap();
        assert!(weighed.known_calories.is_none(), "{item}");
    }
}

#[test]
fn typed_tea_bags_are_a_trace() {
    for item in [
        "tea bags",
        "black tea bags",
        "green tea bag",
        "earl grey tea bag",
    ] {
        let line = ParsedIngredient {
            measurements: vec![Measurement {
                amount: Some("5".into()),
                unit: None,
            }],
            ..ingredient(item, "5", "")
        };
        let result = estimate(&[line], None, 1.0).unwrap();
        assert_eq!(
            result.known_calories.map(|range| range.max),
            Some(0.0),
            "{item}"
        );
    }
}

#[test]
fn a_trace_spoonful_with_a_weight_alternative() {
    let line = |item: &str| ParsedIngredient {
        measurements: vec![
            Measurement {
                amount: Some("1-2".into()),
                unit: Some("tsp".into()),
            },
            Measurement {
                amount: Some("2-4".into()),
                unit: Some("g".into()),
            },
        ],
        ..ingredient(item, "1", "tsp")
    };
    let calories = |item: &str| {
        estimate(&[line(item)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.max)
    };
    // Sumac has no calories to weigh the grams with: the spoonful is a trace.
    assert_eq!(calories("sumac"), Some(0.0));
    // Black pepper does, so the weight alternative counts (4 g x 2.51 kcal/g).
    assert!((calories("black pepper").unwrap() - 10.04).abs() < 1e-9);
}

#[test]
fn bare_cans_use_standard_sizes() {
    let calories = |item: &str, amount: &str, unit: &str| {
        estimate(&[ingredient(item, amount, unit)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.max)
    };
    let per_gram = |item: &str| calories(item, "100", "g").unwrap() / 100.0;
    let can = calories("black beans", "1", "can").unwrap();
    assert!((can - 425.0 * per_gram("black beans")).abs() < 1e-9);
    // A stated size still wins over the standard one.
    let stated = calories("black beans", "1", "28-ounce can").unwrap();
    assert!((stated - 28.0 * 28.349523125 * per_gram("black beans")).abs() < 1e-6);
    // Crushed tomatoes come in two common sizes, so a bare can stays unknown.
    assert_eq!(calories("crushed tomatoes", "1", "can"), None);
}

#[test]
fn per_item_weights_multiply_the_count() {
    let line = |item: &str, measurements: &[(&str, &str)]| ParsedIngredient {
        measurements: measurements
            .iter()
            .map(|(amount, unit)| Measurement {
                amount: Some((*amount).into()),
                unit: Some((*unit).into()),
            })
            .collect(),
        ..ingredient(item, "", "")
    };
    let calories = |line: ParsedIngredient| {
        estimate(&[line], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| (range.min, range.max))
    };
    let grams = |item: &str, grams: f64| {
        calories(ingredient(item, &grams.to_string(), "g"))
            .unwrap()
            .0
    };
    let oz = 28.349523125;
    // "2 (8 oz each) packages cream cheese": two packages of 8 oz.
    let (min, max) = calories(line("cream cheese", &[("2", "package"), ("8", "oz each")])).unwrap();
    assert!((min - grams("cream cheese", 16.0 * oz)).abs() < 1e-9);
    assert_eq!(min, max);
    // A bare count works too, and a ranged count gives a range.
    let (min, max) = calories(line("chicken breast", &[("2-3", ""), ("200", "g each")])).unwrap();
    assert!((min - grams("chicken breast", 400.0)).abs() < 1e-9);
    assert!((max - grams("chicken breast", 600.0)).abs() < 1e-9);
    // A stated weight beats the standard can size.
    let (min, _) = calories(line("black beans", &[("2", "can"), ("10", "oz each")])).unwrap();
    assert!((min - grams("black beans", 20.0 * oz)).abs() < 1e-9);
    // The parser may leave the per-item weight in the note instead:
    // "3 packages (8 oz. each cream cheese)" is unit "package each".
    for unit in ["package each", "packages"] {
        let noted = ParsedIngredient {
            note: Some("8 oz. each cream cheese".into()),
            ..ingredient("cream cheese", "3", unit)
        };
        let (min, _) = calories(noted).unwrap();
        assert!(
            (min - grams("cream cheese", 24.0 * oz)).abs() < 1e-9,
            "{unit}"
        );
    }
    // A per-item amount on the primary is the whole amount.
    let (min, _) = calories(ingredient("granulated sugar", "2", "tbsp each")).unwrap();
    assert_eq!(
        min,
        calories(ingredient("granulated sugar", "2", "tbsp"))
            .unwrap()
            .0
    );
    // A primary that is already a weight or volume counts nothing, so the
    // primary wins and the per-item weight is not used on its own.
    let (min, _) = calories(line("granulated sugar", &[("1", "cup"), ("50", "g each")])).unwrap();
    assert_eq!(
        min,
        calories(ingredient("granulated sugar", "1", "cup"))
            .unwrap()
            .0
    );
    assert_eq!(
        calories(line(
            "cream cheese",
            &[("a few", "package"), ("8", "oz each")]
        )),
        None
    );
}

#[test]
fn a_size_leading_the_item_sizes_each_of_the_count() {
    let calories = |item: &str, amount: &str, unit: &str| {
        estimate(&[ingredient(item, amount, unit)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    let oz = 28.349523125;
    // "2 4-ounce balls burrata": the catalog matches the food without its size.
    assert_eq!(
        calories("4-ounce balls burrata cheese", "2", ""),
        calories("burrata cheese", &(8.0 * oz).to_string(), "g")
    );
    assert_eq!(
        calories("14-oz block extra-firm tofu", "1", ""),
        calories("extra-firm tofu", &(14.0 * oz).to_string(), "g")
    );
    // An inch size weighs as that piece of the food does.
    assert_eq!(
        calories("2\" piece fresh ginger", "1", ""),
        calories("fresh ginger", "1", "2-inch piece")
    );
    assert!(calories("2\" piece fresh ginger", "1", "").is_some());
}

#[test]
fn a_bare_count_weighs_the_piece_the_food_is_named_for() {
    let calories = |item: &str, amount: &str, unit: &str| {
        estimate(&[ingredient(item, amount, unit)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    // USDA weighs a dinner roll "each" (25 g) and "roll" (28 g); a bare count
    // means the roll.
    assert_eq!(
        calories("dinner rolls", "12", ""),
        calories("dinner rolls", "12", "roll")
    );
    assert!(calories("dinner rolls", "12", "").is_some());
    assert_eq!(
        calories("saltine crackers", "42", ""),
        calories("saltine crackers", "42", "cracker")
    );
    assert!(calories("saltine crackers", "42", "").is_some());
}

#[test]
fn a_counted_chipotle_in_adobo_weighs_the_labeled_chile() {
    let calories = |item: &str, amount: &str, unit: &str| {
        estimate(&[ingredient(item, amount, unit)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    // Branded labels give 2 chiles (with their sauce) as 30 g.
    assert!(calories("chipotle peppers in adobo sauce", "2", "").is_some());
    assert_eq!(
        calories("chipotle peppers in adobo sauce", "2", ""),
        calories("chipotle peppers in adobo sauce", "30", "g")
    );
    assert_eq!(
        calories("chipotle chiles en adobo", "1", ""),
        calories("chipotle chile in adobo sauce", "15", "g")
    );
}

#[test]
fn bread_slices_loaves_and_buns_weigh_their_pieces() {
    let calories = |item: &str, amount: &str, unit: &str| {
        estimate(&[ingredient(item, amount, unit)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    // A slice left in the item is the counted piece; FNDDS weighs French
    // bread's slices (thick 96 g, thin 32 g, regular 64 g) and loaf.
    assert!(calories("thick slices sourdough bread", "2", "").is_some());
    assert_eq!(
        calories("thick slices sourdough bread", "2", ""),
        calories("sourdough bread", "192", "g")
    );
    assert_eq!(
        calories("thin slices sourdough bread", "2", ""),
        calories("sourdough bread", "64", "g")
    );
    assert_eq!(
        calories("1-inch thick slices sourdough bread", "4", ""),
        calories("sourdough bread", "256", "g")
    );
    assert_eq!(
        calories("slices sourdough bread", "3", "large"),
        calories("sourdough bread", "288", "g")
    );
    assert_eq!(
        calories("French bread", "1", "loaf"),
        calories("French bread", "470", "g")
    );
    // A food with only a plain slice weighs a thick one as that slice.
    assert_eq!(
        calories("thick slices rye bread", "2", ""),
        calories("rye bread", "2", "slice")
    );
    // Buns and rolls weigh as one, not as unweighable generic rolls.
    assert_eq!(
        calories("burger buns", "8", ""),
        calories("hamburger buns", "8", "")
    );
    assert_eq!(
        calories("ciabatta rolls", "4", ""),
        calories("italian bread", "360", "g")
    );
    assert!(calories("brioche buns", "4", "").is_some());
}

#[test]
fn sheets_weigh_one_sheet() {
    let calories = |item: &str, amount: &str, unit: &str| {
        estimate(&[ingredient(item, amount, unit)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    for (item, grams) in [
        ("puff pastry", 245.0),
        ("phyllo dough", 19.0),
        ("leaf gelatin", 2.2),
        ("wonton wrappers", 8.0),
        ("no-boil lasagna noodles", 17.0),
    ] {
        assert!(calories(item, "2", "sheets").is_some(), "{item}");
        assert_eq!(
            calories(item, "2", "sheets"),
            calories(item, &(2.0 * grams).to_string(), "g"),
            "{item}"
        );
    }
    // A wonton wrapper is the small one, not the egg roll wrapper.
    assert_eq!(
        calories("wonton wrappers", "12", ""),
        calories("wonton wrappers", "96", "g")
    );
}

#[test]
fn leaves_and_sized_heads_weigh_their_pieces() {
    let calories = |item: &str, amount: &str, unit: &str| {
        estimate(&[ingredient(item, amount, unit)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    // Leaves left in the item are the counted piece.
    assert!(calories("kale leaves", "4", "").is_some());
    assert_eq!(
        calories("kale leaves", "4", ""),
        calories("kale", "40", "g")
    );
    assert_eq!(
        calories("romaine leaves", "6", ""),
        calories("romaine lettuce", "6", "leaf")
    );
    // A size alone is the head, not a leaf of that size.
    assert!(calories("cabbage", "1", "medium").is_some());
    assert_eq!(
        calories("cabbage", "1", "medium"),
        calories("cabbage", "908", "g")
    );
}

#[test]
fn whole_produce_and_seafood_weigh_by_size() {
    let calories = |item: &str, amount: &str, unit: &str| {
        estimate(&[ingredient(item, amount, unit)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    // A size or a bare count weighs the whole food: its sized piece, else
    // its typical one.
    for (item, amount, unit, grams) in [
        ("savoy cabbage", "1", "small", 567.0),
        ("savoy cabbage", "1", "large", 680.0),
        ("kohlrabi", "4", "medium", 680.0),
        ("butternut squash", "1", "", 1361.0),
        ("sugar pumpkin", "1", "small", 1474.0),
        ("pumpkins", "2", "", 2948.0),
        ("fresh papaya", "1", "", 330.0),
        ("papaya", "1", "large", 781.0),
        ("blackberries", "3", "large", 15.0),
        ("ahi tuna steak", "1", "small", 227.0),
        ("baby bok choy", "3", "", 339.0),
        ("broccoli", "1", "", 255.0),
        ("broccoli crowns", "2", "", 544.0),
    ] {
        assert!(calories(item, amount, unit).is_some(), "{item}");
        assert_eq!(
            calories(item, amount, unit),
            calories(item, &grams.to_string(), "g"),
            "{amount} {unit} {item}"
        );
    }
    // A crown is no whole head, and a floret no crown.
    assert_eq!(
        calories("broccoli crown", "1", "large"),
        calories("broccoli", "272", "g")
    );
    assert_eq!(
        calories("broccoli florets", "5", ""),
        calories("broccoli", "50", "g")
    );
    assert_eq!(
        calories("broccoli", "1", "bunch"),
        calories("broccoli", "608", "g")
    );
}

#[test]
fn size_piece_and_prep_words_left_in_the_item_still_weigh() {
    let calories = |parsed: ParsedIngredient| {
        estimate(&[parsed], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    let weighed = |item: &str, amount: &str, unit: &str| calories(ingredient(item, amount, unit));
    for (raw, item, amount, unit) in [
        // A size behind a word the parser doesn't read as a unit sizes the
        // count.
        (
            "16 uncooked medium shrimp, peeled and deveined",
            "shrimp",
            "176",
            "g",
        ),
        ("16 uncooked large shrimp", "shrimp", "256", "g"),
        (
            "6 black tiger shrimp (or similar large prawn)",
            "shrimp",
            "66",
            "g",
        ),
        // A size pair is its first size.
        (
            "1 medium/small butternut squash",
            "butternut squash",
            "1361",
            "g",
        ),
        // An unclosed "(" still splits the note off.
        (
            "1 medium butternut squash (peeled, seeded and sliced into 3/4\"",
            "butternut squash",
            "1361",
            "g",
        ),
        // A crown or floret written first is the food's piece.
        ("1 crown of broccoli", "broccoli", "272", "g"),
        ("1 large crown broccoli", "broccoli", "272", "g"),
        ("3 crowns broccoli", "broccoli", "816", "g"),
        (
            "5 roasted broccoli florets (chopped small)",
            "broccoli",
            "50",
            "g",
        ),
        // A bulb that is the whole food.
        (
            "2 small kohlrabi bulbs, peeled and sliced paper thin",
            "kohlrabi",
            "2",
            "small",
        ),
        // What an alternative is for doesn't pass it over.
        (
            "1 small zucchini per person for zoodles* or pho noodles",
            "zucchini",
            "1",
            "small",
        ),
    ] {
        let expected = weighed(item, amount, unit);
        assert!(expected.is_some(), "{item}");
        assert_eq!(calories(parse_ingredient(raw)), expected, "{raw}");
    }
    assert_eq!(
        parse_ingredient("1 medium butternut squash (peeled, seeded and sliced into 3/4\"").item,
        "butternut squash"
    );
}

#[test]
fn shrimp_and_marshmallows_weigh_by_their_size() {
    let calories = |item: &str, amount: &str, unit: &str| {
        estimate(&[ingredient(item, amount, unit)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    // ATK's count per pound; a bare count is medium.
    for (unit, grams) in [
        ("small", 8.0),
        ("medium", 11.0),
        ("", 11.0),
        ("large", 16.0),
        ("extra-large", 20.0),
        ("jumbo", 25.0),
    ] {
        assert!(calories("shrimp", "10", unit).is_some(), "{unit}");
        assert_eq!(
            calories("shrimp", "10", unit),
            calories("shrimp", &(10.0 * grams).to_string(), "g"),
            "{unit}"
        );
    }
    // A large marshmallow is USDA's regular one; jumbo ones are the giant
    // roasters, named or as a size. A bare count could be minis, so it stays
    // unknown.
    assert_eq!(
        calories("marshmallows", "10", "large"),
        calories("marshmallows", "72", "g")
    );
    assert_eq!(
        calories("marshmallows", "8", "jumbo"),
        calories("marshmallows", "224", "g")
    );
    assert_eq!(
        calories("jumbo or s’mores marshmallows", "8", ""),
        calories("marshmallows", "224", "g")
    );
    assert_eq!(calories("mini marshmallows", "20", ""), None);
}

#[test]
fn a_weight_in_the_note_weighs_a_counted_line() {
    let noted = |raw: &str, item: &str, amount: &str, note: &str| ParsedIngredient {
        measurements: vec![Measurement {
            amount: Some(amount.into()),
            unit: None,
        }],
        note: Some(note.into()),
        raw: Some(raw.into()),
        ..ingredient(item, "", "")
    };
    let calories = |line: ParsedIngredient| {
        estimate(&[line], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| (range.min, range.max))
    };
    let grams = |item: &str, grams: f64| {
        calories(ingredient(item, &grams.to_string(), "g"))
            .unwrap()
            .0
    };
    let pound = 453.59237;
    // Four packages of 8 oz, not 8 oz in all.
    let (min, max) = calories(noted(
        "4 (8 ounce packages) cream cheese",
        "cream cheese",
        "4",
        "8 ounce packages",
    ))
    .unwrap();
    assert!((min - grams("cream cheese", 32.0 * 28.349523125)).abs() < 1e-9);
    assert_eq!(min, max);
    // The roast's own weight, as a range.
    let (min, max) = calories(noted(
        "1 3-to-4-rib roast of beef, approximately 8 to 10 pounds",
        "3-to-4-rib roast of beef",
        "1",
        "approximately 8 to 10 pounds",
    ))
    .unwrap();
    assert!((min - grams("3-to-4-rib roast of beef", 8.0 * pound)).abs() < 1e-6);
    assert!((max - grams("3-to-4-rib roast of beef", 10.0 * pound)).abs() < 1e-6);
    // The note only weighs what the catalog can't: a food with a piece
    // weight is counted by it.
    let fillets = |note: &str| {
        calories(noted(
            "2 salmon fillets, about 1 lb",
            "salmon fillets",
            "2",
            note,
        ))
    };
    assert_eq!(fillets("about 1 lb"), fillets("skin on"));
    assert!(fillets("skin on").is_some());
}

#[test]
fn a_weight_per_person_weighs_each_portion() {
    let calories = |raw: &str| {
        estimate(&[parse_ingredient(raw)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    assert_eq!(
        calories("2 portions cooked Japanese short-grain rice (approx. 150 g per person)"),
        calories("300 g cooked Japanese short-grain rice")
    );
    assert_eq!(
        calories("3 servings cooked white rice (100 g per serving)"),
        calories("300 g cooked white rice")
    );
    // A portion or serving with no weight can't be weighed.
    assert_eq!(
        calories("2 portions cooked Japanese short-grain rice"),
        None
    );
    assert_eq!(
        calories("2 servings cooked Japanese short-grain rice"),
        None
    );
}

#[test]
fn alternatives_count_the_first_and_say_so() {
    let line = |item: &str| {
        let result = estimate(&[ingredient(item, "100", "g")], None, 1.0).unwrap();
        (
            result.known_calories.map(|range| range.max),
            result.lines[0].text.clone(),
        )
    };
    let (calories, text) = line("sour cream or plain yogurt");
    assert_eq!(calories, line("sour cream").0);
    assert_eq!(
        text,
        format!("{} (assumed sour cream)", line("sour cream").1)
    );
    // A name that resolves on its own carries no note.
    assert!(!line("sour cream").1.contains("assumed"));
    // A note that overrides the alternative's food ("cooked" rules out the
    // catalog's unprepared frozen corn) leaves nothing assumed.
    let cooked = ParsedIngredient {
        note: Some("cooked".into()),
        ..ingredient("frozen or canned corn", "1", "cup")
    };
    let result = estimate(&[cooked], None, 1.0).unwrap();
    assert!(
        !result.lines[0].text.contains("assumed"),
        "{}",
        result.lines[0].text
    );
}

#[test]
fn estimated_weights_fill_catalog_gaps_and_say_so() {
    let capers = match catalog::resolve("capers") {
        catalog::Resolution::Entry { entry, .. } => entry,
        other => panic!("capers: {other:?}"),
    };
    assert!(
        capers.grams_per_cup.is_none(),
        "pick a food without a density"
    );
    let result = estimate(&[ingredient("capers", "2", "tbsp")], None, 1.0).unwrap();
    match catalog::estimated_weight(capers, "cup") {
        // Counted at the estimated density, and labeled.
        Some(Some(grams)) => {
            let kcal = capers.kcal_per_100g.unwrap() * grams / 8.0 / 100.0;
            assert!((result.known_calories.unwrap().max - kcal).abs() < 1e-9);
            assert!(
                result.lines[0].text.ends_with("(estimated weight)"),
                "{}",
                result.lines[0].text
            );
            assert!(result.weight_gaps.is_empty());
        }
        // The model said there's no typical weight: unknown, and not a gap
        // to ask about again.
        Some(None) => {
            assert!(result.known_calories.is_none());
            assert!(result.weight_gaps.is_empty());
        }
        // Unknown, and reported for `make catalog-fill-weights`.
        None => {
            assert!(result.known_calories.is_none());
            assert_eq!(
                result.weight_gaps,
                vec![WeightKey {
                    food: capers.id.clone(),
                    unit: "cup".into(),
                }]
            );
        }
    }
    // A measurement the catalog can weigh wins over an estimated one, and
    // reports nothing.
    let weighed = ParsedIngredient {
        measurements: vec![
            Measurement {
                amount: Some("2".into()),
                unit: Some("tbsp".into()),
            },
            Measurement {
                amount: Some("18".into()),
                unit: Some("g".into()),
            },
        ],
        ..ingredient("capers", "2", "tbsp")
    };
    let result = estimate(&[weighed], None, 1.0).unwrap();
    assert!(!result.lines[0].text.contains("estimated"));
    assert!(result.weight_gaps.is_empty());
    // Only known units are estimated, so the fill asks a bounded set of
    // questions.
    for unit in ["1/2-inch pieces", "zqfrobs", "glugs"] {
        let junk = estimate(&[ingredient("capers", "2", unit)], None, 1.0).unwrap();
        assert!(junk.weight_gaps.is_empty(), "{unit}");
    }
}

/// Every estimated weight in the catalog weighs a line of its food, so none is
/// stale after a catalog edit (an entry that gained its own weight, or a
/// renamed one).
#[test]
fn every_estimated_weight_is_used() {
    let file: catalog::EstimatedWeightsFile =
        serde_json::from_str(catalog::ESTIMATED_WEIGHTS_JSON).unwrap();
    let mut unused = Vec::new();
    for (id, units) in &file.weights {
        let entry = match catalog::resolve(id) {
            catalog::Resolution::Entry { entry, .. } if entry.id == *id => entry,
            // A USDA description several foods share names none of them.
            _ => continue,
        };
        for (unit, weight) in units {
            let Some(grams) = weight.grams else { continue };
            // Twenty, past the few pieces of a trace food that are
            // negligible.
            let result = estimate(&[ingredient(id, "20", unit)], None, 1.0).unwrap();
            let kcal = entry.kcal_per_100g.unwrap_or_default() * grams * 20.0 / 100.0;
            let counted = result
                .known_calories
                .is_some_and(|known| (known.max - kcal).abs() < 1e-6);
            if !counted || !result.lines[0].text.contains("estimated weight") {
                unused.push(format!("{id} per {unit}: {}", result.lines[0].text));
            }
        }
    }
    assert!(unused.is_empty(), "unused estimated weights: {unused:#?}");
}

#[test]
fn counted_nuts_and_sausages_weigh_by_the_piece() {
    let calories = |item: &str, amount: &str, unit: &str| {
        estimate(&[ingredient(item, amount, unit)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    // A bare count of nuts is that many nuts (halves, for pecans and
    // walnuts), of hot dogs or smoked sausages that many links, and of candy
    // that many pieces.
    for (item, amount, grams) in [
        ("cashew nuts", "15", 22.5),
        ("whole cashewnuts", "4", 6.0),
        ("pecan halves", "12", 18.0),
        ("walnut halves", "12", 24.0),
        ("hot dogs", "8", 416.0),
        ("hot dog sausages", "3", 156.0),
        ("smoked sausages", "4", 272.0),
        ("caramels", "28", 282.8),
        ("gummy worms", "24", 177.6),
        ("fun size butterfinger candy bars", "2", 36.0),
    ] {
        let counted = calories(item, amount, "").expect(item);
        let weighed = calories(item, &grams.to_string(), "g").expect(item);
        assert!((counted - weighed).abs() < 1e-6, "{amount} {item}");
    }
}

#[test]
fn caramels_are_candies_not_caramel_sauce() {
    let calories = |item: &str, amount: &str, unit: &str| {
        estimate(&[ingredient(item, amount, unit)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    assert_ne!(
        calories("caramels", "100", "g"),
        calories("caramel sauce", "100", "g")
    );
}

#[test]
fn pieces_usda_weighs_under_another_name() {
    let calories = |item: &str, amount: &str, unit: &str| {
        estimate(&[ingredient(item, amount, unit)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    for (item, amount, unit, grams) in [
        // SR names the piece by its alternatives: "wedge or slice".
        ("lemon", "2", "wedge", 14.0),
        ("lemon", "3", "slices", 21.0),
        // A bundle is a bunch.
        ("scallions", "1", "bundle", 105.0),
        ("green onion", "2", "bundles", 210.0),
        // A stick of rhubarb is a stalk.
        ("rhubarb", "2", "stick", 102.0),
        // Instant ramen is a package of noodles.
        ("instant ramen noodles", "1", "package", 81.0),
        ("instant ramen noodles", "2", "", 162.0),
        // Canned anchovies are fillets.
        ("anchovy", "1", "fillet", 4.0),
    ] {
        let counted = calories(item, amount, unit).expect(item);
        let weighed = calories(item, &grams.to_string(), "g").expect(item);
        assert!((counted - weighed).abs() < 1e-6, "{amount} {unit} {item}");
    }
}

#[test]
fn ginger_pieces_sized_in_the_note_or_item_weigh_by_the_inch() {
    let calories = |raw: &str| {
        estimate(&[parse_ingredient(raw)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    let inches = |size: &str| calories(&format!("1 {size}-inch piece fresh ginger"));
    for (raw, size) in [
        // The size in the note of a counted piece.
        ("1 (2-inch-long) piece fresh ginger", "2"),
        (
            "1 piece (1 1/2 inches) fresh ginger, peeled and chopped",
            "1 1/2",
        ),
        (
            "1 piece fresh ginger ((approx 1/2 inch) peeled and chopped)",
            "1/2",
        ),
        ("1 piece peeled ginger (2\")", "2"),
        // The unit left in the item.
        ("1 inch knob fresh ginger ((sliced thin))", "1"),
        ("1 inch piece ginger", "1"),
        ("1/2 inch ginger (juliennes)", "1/2"),
        ("1.5 inches fresh ginger ((sliced into 8 coins))", "1.5"),
    ] {
        assert!(calories(raw).is_some(), "{raw}");
        assert_eq!(calories(raw), inches(size), "{raw}");
    }
    // A leading inch size behind a piece unit sizes each piece.
    assert_eq!(
        calories("2 piece 1-inch fresh ginger root"),
        calories("2 1-inch pieces fresh ginger root")
    );
    // A note that sizes nothing leaves a bare piece unknown, as before.
    assert_eq!(calories("1 piece fresh ginger, peeled"), None);
    // A stated weight still beats the inch size.
    assert_eq!(
        calories("1/2 inch (5 grams) ginger"),
        calories("5 grams ginger")
    );
}

#[test]
fn counted_olives_pickles_and_artichoke_hearts_weigh_by_their_piece() {
    let calories = |raw: &str| {
        estimate(&[parse_ingredient(raw)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    for (counted, weighed) in [
        ("16 kalamata olives", "80 g kalamata olives"),
        ("6 pepperoncini", "90 g pepperoncini"),
        (
            "3 marinated artichoke hearts",
            "90 g marinated artichoke hearts",
        ),
        // Small pickles, not the default large one.
        ("6 baby dill pickles", "180 g baby dill pickles"),
        ("12 hamburger pickle chips", "84 g hamburger pickle chips"),
        ("10 pickled jalapeño slices", "20 g pickled jalapeño slices"),
    ] {
        let (counted_kcal, weighed_kcal) = (calories(counted), calories(weighed));
        assert!(counted_kcal.is_some(), "{counted}");
        assert!(
            (counted_kcal.unwrap() - weighed_kcal.unwrap()).abs() < 1e-6,
            "{counted}"
        );
    }
}

#[test]
fn containers_and_hedged_weights_read_their_stated_size() {
    let calories = |raw: &str| {
        estimate(&[parse_ingredient(raw)], None, 1.0)
            .unwrap()
            .known_calories
    };
    for (stated, weighed) in [
        // A metric volume on the container.
        ("1 750-ml bottle red wine", "750 ml red wine"),
        // A space typed after the hyphen.
        (
            "1 12- ounce beer, or equivalent amount of broth",
            "12 oz beer",
        ),
        // A range hyphenated through.
        (
            "10-to-12-ounce (285-to-340-gram) bag semisweet chocolate chips",
            "10 to 12 oz semisweet chocolate chips",
        ),
        // A weight leading the item after a package.
        (
            "1 package, 9 oz. chocolate wafer cookies (such as Nabisco)",
            "9 oz chocolate wafer cookies",
        ),
        // A noted weight that runs on into the item.
        (
            "1  package (.25 ounce powdered gelatin)",
            "0.25 oz powdered gelatin",
        ),
        (
            "6  packets (.25 oz. each powdered gelatin)",
            "1.5 oz powdered gelatin",
        ),
        // "roughly" hedges a weight like "about".
        (
            "1 kabocha squash, roughly 2.5 – 3 pounds",
            "2.5 to 3 pounds kabocha squash",
        ),
    ] {
        let (stated_kcal, weighed_kcal) = (calories(stated), calories(weighed));
        let (stated_kcal, weighed_kcal) = (stated_kcal.expect(stated), weighed_kcal.unwrap());
        assert!(
            (stated_kcal.min - weighed_kcal.min).abs() < 1e-6
                && (stated_kcal.max - weighed_kcal.max).abs() < 1e-6,
            "{stated}: {stated_kcal:?} vs {weighed_kcal:?}"
        );
    }
}

#[test]
fn bread_pieces_baguette_slices_and_whole_wheat_tortillas_weigh_by_the_piece() {
    let calories = |raw: &str| {
        estimate(&[parse_ingredient(raw)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    for (counted, weighed) in [
        // A piece of bread is a slice.
        (
            "2 pieces sourdough or whole grain bread, toasted",
            "128 g sourdough bread",
        ),
        ("2 pieces of toasted bread", "58 g toasted bread"),
        // A baguette slice is small.
        (
            "24 1/4-inch-thick baguette bread slices",
            "240 g baguette slices",
        ),
        // A loaf of pita is one pita.
        ("6 loaves of pita bread", "360 g pita bread"),
        ("4 whole wheat tortillas", "180 g whole wheat tortillas"),
        (
            "2 large whole wheat tortillas",
            "142 g whole wheat tortillas",
        ),
    ] {
        let (counted_kcal, weighed_kcal) = (calories(counted), calories(weighed));
        assert!(counted_kcal.is_some(), "{counted}");
        assert!(
            (counted_kcal.unwrap() - weighed_kcal.unwrap()).abs() < 1e-6,
            "{counted}: {counted_kcal:?} vs {weighed_kcal:?}"
        );
    }
}

#[test]
fn seasoning_mix_packets_and_bouillon_cubes_weigh_by_the_package() {
    let calories = |raw: &str| {
        estimate(&[parse_ingredient(raw)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    for (counted, weighed) in [
        // An envelope, packet or package of a mix is the same container.
        ("1 envelope taco seasoning", "28 g taco seasoning"),
        ("1 packet taco seasoning", "28 g taco seasoning"),
        ("1 envelope brown gravy mix", "24 g brown gravy mix"),
        ("1 package ranch mix", "28 g ranch mix"),
        // USDA's gelatin envelope answers for a package.
        ("2 packages unflavored gelatin", "14 g unflavored gelatin"),
        // A bare count of bouillon is cubes.
        (
            "1 chicken bouillon (cube or powder)",
            "4 g chicken bouillon",
        ),
    ] {
        let (counted_kcal, weighed_kcal) = (calories(counted), calories(weighed));
        assert!(counted_kcal.is_some(), "{counted}");
        assert!(
            (counted_kcal.unwrap() - weighed_kcal.unwrap()).abs() < 1e-6,
            "{counted}: {counted_kcal:?} vs {weighed_kcal:?}"
        );
    }
}

#[test]
fn balls_of_dough_and_cheese_and_blocks_of_cream_cheese_weigh_by_the_piece() {
    let calories = |raw: &str| {
        estimate(&[parse_ingredient(raw)], None, 1.0)
            .unwrap()
            .known_calories
            .map(|range| range.min)
    };
    for (counted, weighed) in [
        ("1 ball no-knead pizza dough", "454 g pizza dough"),
        // A bare count of pizza dough is a ball.
        ("1 fresh pizza dough", "454 g pizza dough"),
        ("1 ball burrata, torn", "227 g burrata"),
        ("2 large balls of burrata", "454 g burrata"),
        ("1 ball fresh mozzarella", "226 g fresh mozzarella"),
        ("1/2 block cream cheese", "113.5 g cream cheese"),
    ] {
        let (counted_kcal, weighed_kcal) = (calories(counted), calories(weighed));
        assert!(counted_kcal.is_some(), "{counted}");
        assert!(
            (counted_kcal.unwrap() - weighed_kcal.unwrap()).abs() < 1e-6,
            "{counted}: {counted_kcal:?} vs {weighed_kcal:?}"
        );
    }
}
