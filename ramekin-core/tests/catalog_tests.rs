use ramekin_core::catalog::{food, grams_per_cup, resolve, rewrite, version, Resolution, Via};

fn density(item: &str) -> f64 {
    grams_per_cup(item).unwrap_or_else(|| panic!("no density for {item:?}"))
}

/// The USDA food supplying calories for a name.
fn fdc_id(item: &str) -> Option<u32> {
    match resolve(item) {
        Resolution::Entry { entry, .. } => entry.fdc_id,
        other => panic!("{item:?} did not resolve: {other:?}"),
    }
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.1, "{actual} != {expected}");
}

#[test]
fn density_lookup_chain() {
    // USDA stripped name, curated alias, modifiers, case, plural, unknown.
    assert!(grams_per_cup("salt, table").is_some());
    for item in ["flour", "sugar", "butter", "FLOUR", "Butter"] {
        assert!(grams_per_cup(item).is_some(), "{item}");
    }
    assert!(grams_per_cup("softened butter").is_some());
    assert!(grams_per_cup("melted butter").is_some());
    assert!(
        grams_per_cup("onion").is_some(),
        "plural fallback to onions"
    );
    assert_eq!(grams_per_cup("unicorn tears"), None);
    assert_eq!(grams_per_cup("mystery powder"), None);
}

#[test]
fn curated_aliases_have_densities() {
    for item in [
        "baking powder",
        "baking soda",
        "instant yeast",
        "rapid-rise yeast",
        "instant dry yeast",
        "bread machine yeast",
        "cinnamon",
        "ground cinnamon",
        "garlic powder",
        "yellow onion",
        "red onion",
        "buttermilk",
        "greek yogurt",
        "mayo",
        "mayonnaise",
        "mustard",
        "yellow mustard",
        "dijon mustard",
        "water",
        "soy sauce",
        "ground cumin",
        "dried oregano",
        "pure vanilla extract",
        "Worcestershire sauce",
        "fresh lemon juice",
        "tomato paste",
        "sesame oil",
        "vanilla",
        "rice vinegar",
        "cumin",
        "oregano",
        "toasted sesame oil",
        "apple cider vinegar",
        "red wine vinegar",
        "balsamic vinegar",
        "white vinegar",
        "white wine vinegar",
        "tamari",
        "Japanese soy sauce (koikuchi shoyu)",
        "all purpose flour",
        "ketchup",
        "fresh lime juice",
        "freshly squeezed lemon juice",
        "orange juice",
        "boiling water",
        "hot water",
        "ice water",
        "cider vinegar",
        "distilled white vinegar",
        "half and half",
        "half-and-half",
        "cilantro",
        "fresh cilantro",
        "hoisin sauce",
        "raisins",
        "golden raisins",
        "tahini",
        "sesame seeds",
        "pine nuts",
        "pumpkin pie spice",
        "freshly grated parmesan",
        "finely grated parmesan cheese",
        "finely shredded parmesan cheese",
        "shredded mozzarella",
        "shredded mozzarella cheese",
        "chicken stock",
        "low-sodium chicken broth",
        "vegetable broth",
        "beef broth",
        "white wine",
        "Shaoxing wine",
        "corn syrup",
    ] {
        assert!(grams_per_cup(item).is_some(), "{item}");
    }
}

#[test]
fn curated_densities_keep_their_values() {
    assert_close(density("salt"), 137.0);
    assert_close(density("kosher salt"), 137.0);
    assert_close(density("table salt"), 292.0);
    assert_close(density("fish sauce"), 288.0);
    assert_close(density("chicken broth"), 249.0);
    assert_close(density("light corn syrup"), 341.0);
    assert_close(density("oyster sauce"), 288.0);
    assert_close(density("mirin"), 240.0);
    assert_close(density("almond extract"), 208.0);
    assert_close(density("dry white wine"), 236.0);
    assert_close(density("sake"), 240.0);
    assert_close(density("grated parmesan cheese"), 100.0);
    assert_close(density("shredded parmesan cheese"), 80.0);
    // Manual baking values override the linked USDA food's density.
    assert_close(density("all-purpose flour"), 125.0);
    assert_close(density("butter"), 227.0);
}

#[test]
fn some_foods_resolve_without_a_density() {
    // Grind and crystal size make volume-to-weight unreliable, but the food is
    // still known for calories.
    for item in [
        "sea salt",
        "fine salt",
        "black pepper",
        "ground black pepper",
        "freshly ground black pepper",
        "pepper",
    ] {
        assert_eq!(grams_per_cup(item), None, "{item}");
        assert!(fdc_id(item).is_some(), "{item}");
    }
}

#[test]
fn shared_defaults_and_ambiguous_names() {
    assert_eq!(fdc_id("sugar"), fdc_id("granulated sugar"));
    assert_eq!(fdc_id("flour"), fdc_id("all-purpose flour"));
    assert_eq!(fdc_id("butter"), Some(173430), "unsalted butter");
    assert_eq!(fdc_id("salted butter"), Some(173410));
    assert_eq!(fdc_id("oil"), fdc_id("vegetable oil"));
    assert_eq!(fdc_id("milk"), fdc_id("whole milk"));
    assert_eq!(
        food(fdc_id("salt").unwrap()).unwrap().kcal_per_100g,
        Some(0.0)
    );
    for item in ["rice", "chicken", "cheese", "yogurt"] {
        assert!(
            matches!(resolve(item), Resolution::Ambiguous),
            "{item} stays ambiguous"
        );
        assert_eq!(grams_per_cup(item), None);
    }
    // Some foods have no USDA equivalent, so calories stay unknown.
    assert_eq!(fdc_id("mirin"), None);
}

#[test]
fn resolution_reports_how_it_matched() {
    let via = |item| match resolve(item) {
        Resolution::Entry { via, .. } => via,
        other => panic!("{item:?} did not resolve: {other:?}"),
    };
    assert_eq!(via("garlic, raw"), Via::Exact);
    assert_eq!(
        via("  Dry   White  Wine "),
        Via::Exact,
        "whitespace collapses"
    );
    assert_eq!(via("yellow onions"), Via::Plural);
    assert_eq!(via("room temperature butter"), Via::Modifiers);
    assert!(matches!(resolve("unicorn tears"), Resolution::Unresolved));
}

#[test]
fn rewrites_and_version() {
    assert_eq!(rewrite("salt"), Some("kosher salt"));
    assert_eq!(rewrite(" Salt "), Some("kosher salt"));
    assert_eq!(rewrite("kosher salt"), None);
    assert_eq!(rewrite("flour"), None);
    assert!(version().starts_with("catalog-v1-sr2018-"));
    assert_eq!(version(), version());
}
