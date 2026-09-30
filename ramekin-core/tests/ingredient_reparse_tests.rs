use ramekin_core::ingredient_parser::{reparse_stored, Measurement, ParsedIngredient};

fn stored(
    item: &str,
    measurements: Vec<(&str, Option<&str>)>,
    note: Option<&str>,
) -> ParsedIngredient {
    ParsedIngredient {
        item: item.into(),
        measurements: measurements
            .into_iter()
            .map(|(amount, unit)| Measurement {
                amount: Some(amount.into()),
                unit: unit.map(Into::into),
            })
            .collect(),
        note: note.map(Into::into),
        raw: None,
        section: Some("Sauce".into()),
    }
}

#[test]
fn junk_items_move_notes_out_and_keep_stored_measurements() {
    let updated = reparse_stored(&stored(
        "chickpeas, drained, rinsed",
        vec![("2", Some("cup"))],
        Some("from 1 can"),
    ))
    .unwrap();
    assert_eq!(updated.item, "chickpeas");
    assert_eq!(updated.note.as_deref(), Some("drained, rinsed, from 1 can"));
    assert_eq!(updated.measurements.len(), 1);
    assert_eq!(updated.measurements[0].unit.as_deref(), Some("cup"));
    assert_eq!(updated.section.as_deref(), Some("Sauce"));
}

#[test]
fn amounts_in_the_item_are_used_only_when_none_were_stored() {
    let updated = reparse_stored(&stored("about 7 cloves garlic, minced", vec![], None)).unwrap();
    assert_eq!(updated.item, "garlic");
    assert_eq!(updated.measurements[0].amount.as_deref(), Some("7"));
    assert_eq!(updated.measurements[0].unit.as_deref(), Some("clove"));
    assert!(updated.note.as_deref().unwrap().contains("minced"));
}

#[test]
fn clean_items_and_numbered_names_are_left_alone() {
    assert!(reparse_stored(&stored("garlic", vec![("2", Some("clove"))], None)).is_none());
    assert!(reparse_stored(&stored(
        "85% lean ground beef",
        vec![("1", Some("lb"))],
        None
    ))
    .is_none());
    assert!(reparse_stored(&stored("5- to 6-inch cubanelle chiles", vec![], None)).is_none());
}
