use ramekin_core::catalog::{
    candidates, learned_key_resolves, resolve_line_with, unlearned_name, EstimatedFood, Learned,
    LearnedTarget, Resolution, Via,
};
use ramekin_core::ingredient_categorizer::categorize_with;
use ramekin_core::ingredient_parser::{Measurement, ParsedIngredient};
use ramekin_core::nutrition::{estimate, estimate_with, Weights};

fn learned(pairs: &[(&str, LearnedTarget)]) -> Learned {
    pairs
        .iter()
        .map(|(name, target)| (name.to_string(), target.clone()))
        .collect()
}

#[test]
fn only_names_the_catalog_misses_are_learned() {
    assert_eq!(unlearned_name("Moon Dust"), Some("moon dust".to_string()));
    assert_eq!(unlearned_name("garlic"), None);
    // Ambiguous names are sent too, for the food a recipe most likely means.
    assert_eq!(unlearned_name("cheese"), Some("cheese".to_string()));
}

fn grams_line(item: &str, amount: &str, unit: &str) -> ParsedIngredient {
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
fn ambiguous_names_take_a_learned_default_and_say_so() {
    let line = grams_line("cheese", "100", "g");
    let without = estimate(std::slice::from_ref(&line), None, 1.0).unwrap();
    assert_eq!(without.lines[0].text, "Could be several foods");
    let learned = learned(&[("cheese", LearnedTarget::Entry("cheddar cheese".into()))]);
    let with = estimate_with(
        std::slice::from_ref(&line),
        None,
        1.0,
        &learned,
        &Weights::new(),
    )
    .unwrap();
    let cheddar = estimate(&[grams_line("cheddar cheese", "100", "g")], None, 1.0).unwrap();
    assert_eq!(with.known_calories, cheddar.known_calories);
    assert!(
        with.lines[0].text.ends_with("(assumed cheddar cheese)"),
        "{}",
        with.lines[0].text
    );
    // An "unknown" answer leaves it ambiguous.
    let unknown = self::learned(&[("cheese", LearnedTarget::Unknown)]);
    assert!(matches!(
        resolve_line_with("cheese", None, &unknown),
        Resolution::Ambiguous(_)
    ));
}

#[test]
fn estimated_foods_count_with_the_model_numbers_and_say_so() {
    let estimate_of = EstimatedFood {
        kcal_per_100g: 250.0,
        grams_per_cup: Some(120.0),
        grams_per_piece: Some(30.0),
    };
    let learned = learned(&[("moon dust", LearnedTarget::Estimate(estimate_of))]);
    // Still not a catalog entry, so densities and categories don't use it.
    assert!(matches!(
        resolve_line_with("moon dust", None, &learned),
        Resolution::Unresolved
    ));
    for (amount, unit, grams) in [("100", "g", 100.0), ("1", "cup", 120.0), ("2", "", 60.0)] {
        let line = grams_line("Moon Dust", amount, unit);
        let result = estimate_with(&[line], None, 1.0, &learned, &Weights::new()).unwrap();
        assert!(
            (result.known_calories.unwrap().max - grams * 2.5).abs() < 1e-9,
            "{amount} {unit}"
        );
        assert!(
            result.lines[0].text.ends_with("(estimated calories)"),
            "{}",
            result.lines[0].text
        );
    }
    // A unit it gave no weight for is a weight gap like any food's, and once
    // estimated the line says both numbers are the model's.
    let line = grams_line("moon dust", "2", "jars");
    let result = estimate_with(
        std::slice::from_ref(&line),
        None,
        1.0,
        &learned,
        &Weights::new(),
    )
    .unwrap();
    assert!(result.known_calories.is_none());
    assert_eq!(result.weight_gaps.len(), 1);
    assert_eq!(result.weight_gaps[0].unit, "jar");
    let weights = Weights::from([(result.weight_gaps[0].clone(), 50.0)]);
    let weighed = estimate_with(&[line], None, 1.0, &learned, &weights).unwrap();
    assert!((weighed.known_calories.unwrap().max - 250.0).abs() < 1e-9);
    assert!(
        weighed.lines[0]
            .text
            .ends_with("(estimated calories; estimated weight)"),
        "{}",
        weighed.lines[0].text
    );
    // A zero-calorie estimate is negligible on any line, weighable or not.
    let diet = self::learned(&[(
        "moon soda",
        LearnedTarget::Estimate(EstimatedFood {
            kcal_per_100g: 0.0,
            grams_per_cup: None,
            grams_per_piece: None,
        }),
    )]);
    let result = estimate_with(
        &[grams_line("moon soda", "2", "bottles")],
        None,
        1.0,
        &diet,
        &Weights::new(),
    )
    .unwrap();
    assert_eq!(result.lines[0].text, "Negligible (estimated calories)");
    // An estimated fat listed for frying follows the frying-medium rule.
    let fat = self::learned(&[(
        "vanaspati",
        LearnedTarget::Estimate(EstimatedFood {
            kcal_per_100g: 880.0,
            grams_per_cup: Some(205.0),
            grams_per_piece: None,
        }),
    )]);
    let frying = ParsedIngredient {
        note: Some("for frying".into()),
        ..grams_line("vanaspati", "2", "cups")
    };
    let result = estimate_with(&[frying], None, 1.0, &fat, &Weights::new()).unwrap();
    assert!(result.known_calories.is_none());
    assert_eq!(
        result.unknown_ingredients[0].reason,
        "Frying oil: only part of it is absorbed"
    );
    let browning = ParsedIngredient {
        note: Some("for frying".into()),
        ..grams_line("vanaspati", "1", "tbsp")
    };
    let result = estimate_with(&[browning], None, 1.0, &fat, &Weights::new()).unwrap();
    assert!(result.known_calories.is_some());
    assert!(result.weight_gaps.is_empty());
}

#[test]
fn learned_entries_resolve_through_the_catalog() {
    let learned = learned(&[
        (
            "moon sugar",
            LearnedTarget::Entry("granulated sugar".into()),
        ),
        ("serving platter", LearnedTarget::NotFood),
        ("moon dust", LearnedTarget::Unknown),
    ]);
    match resolve_line_with("Moon Sugar", None, &learned) {
        Resolution::Entry { entry, via } => {
            assert_eq!(via, Via::Learned);
            assert!(entry.fdc_id.is_some());
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        resolve_line_with("serving platter", None, &learned),
        Resolution::NotFood
    ));
    assert!(matches!(
        resolve_line_with("moon dust", None, &learned),
        Resolution::Unresolved
    ));
    // Nothing stored yet: still unknown.
    assert!(matches!(
        resolve_line_with("comet tail", None, &learned),
        Resolution::Unresolved
    ));
    // The committed catalog always wins.
    let garlic = learned_with_override();
    match resolve_line_with("garlic", None, &garlic) {
        Resolution::Entry { via, .. } => assert_ne!(via, Via::Learned),
        other => panic!("{other:?}"),
    }
}

fn learned_with_override() -> Learned {
    learned(&[("garlic", LearnedTarget::NotFood)])
}

#[test]
fn estimates_and_categories_use_learned_names() {
    let line = ParsedIngredient {
        item: "moon sugar".into(),
        measurements: vec![Measurement {
            amount: Some("100".into()),
            unit: Some("g".into()),
        }],
        note: None,
        raw: None,
        section: None,
    };
    let without = estimate(std::slice::from_ref(&line), None, 1.0).unwrap();
    assert!(without.known_calories.is_none());
    let learned = learned(&[(
        "moon sugar",
        LearnedTarget::Entry("granulated sugar".into()),
    )]);
    let with = estimate_with(
        std::slice::from_ref(&line),
        None,
        1.0,
        &learned,
        &Weights::new(),
    )
    .unwrap();
    assert_eq!(with.known_calories.unwrap().min, 387.0);

    assert_eq!(categorize_with("moon sugar", &learned), "Baking");
    let platter = self::learned(&[("serving platter", LearnedTarget::NotFood)]);
    assert_eq!(categorize_with("serving platter", &platter), "Other");
}

#[test]
fn learned_names_never_lose_a_keyword_category() {
    use ramekin_core::ingredient_categorizer::categorize;
    // A learned non-food is categorized like a committed one, by keywords.
    let item = "any vegetables you have stashed in your stock bag";
    assert_ne!(categorize(item), "Other");
    let not_food = learned(&[(item, LearnedTarget::NotFood)]);
    assert_eq!(categorize_with(item, &not_food), categorize(item));
    // A learned key with no catalog category and no keyword keeps the item's
    // own keyword category rather than becoming "Other".
    let key = "abiyuch";
    assert!(learned_key_resolves(key));
    assert_eq!(categorize(key), "Other");
    let item = "zzqq carrot medley";
    assert_eq!(categorize(item), "Produce");
    let entry = learned(&[(item, LearnedTarget::Entry(key.into()))]);
    assert_eq!(categorize_with(item, &entry), "Produce");
}

#[test]
fn candidates_share_words_with_the_name() {
    let found = candidates("garam masala spice blend", 10);
    assert!(!found.is_empty());
    assert!(found.len() <= 10);
    assert!(
        found.iter().all(|key| key.contains("spice")
            || key.contains("blend")
            || key.contains("garam")
            || key.contains("masala")),
        "{found:?}"
    );
    assert!(candidates("the of and", 10).is_empty());
}

#[test]
fn a_cooked_note_applies_to_the_learned_key() {
    let learned = learned(&[(
        "moon rice",
        LearnedTarget::Entry("rice, white, long-grain, regular, raw, enriched".into()),
    )]);
    let dry = match resolve_line_with("moon rice", None, &learned) {
        Resolution::Entry { entry, .. } => entry.id.clone(),
        other => panic!("{other:?}"),
    };
    // The dry-rice key with a "cooked" note never means dry rice.
    match resolve_line_with("moon rice", Some("cooked"), &learned) {
        Resolution::Entry { entry, .. } => assert_ne!(entry.id, dry),
        Resolution::Unresolved => {}
        other => panic!("{other:?}"),
    }
}

#[test]
fn learned_keys_are_checked_against_the_current_catalog() {
    assert!(learned_key_resolves("garlic"));
    assert!(!learned_key_resolves("moon dust"));
    // Ambiguous names don't name one entry either.
    assert!(!learned_key_resolves("cheese"));
}
