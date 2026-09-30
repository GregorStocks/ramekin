use ramekin_core::catalog::{
    candidates, resolve_line_with, unlearned_name, Learned, LearnedTarget, Resolution, Via,
};
use ramekin_core::ingredient_categorizer::categorize_with;
use ramekin_core::ingredient_parser::{Measurement, ParsedIngredient};
use ramekin_core::nutrition::{estimate, estimate_with};

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
    // Ambiguous on purpose, never sent to the LLM.
    assert_eq!(unlearned_name("cheese"), None);
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
    let with = estimate_with(std::slice::from_ref(&line), None, 1.0, &learned).unwrap();
    assert_eq!(with.known_calories.unwrap().min, 387.0);

    assert_eq!(categorize_with("moon sugar", &learned), "Baking");
    let platter = self::learned(&[("serving platter", LearnedTarget::NotFood)]);
    assert_eq!(categorize_with("serving platter", &platter), "Other");
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
