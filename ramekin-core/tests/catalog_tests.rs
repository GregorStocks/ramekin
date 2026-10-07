use ramekin_core::catalog::{
    category, chosen_alternative, food, grams_per_cup, grams_per_piece, line_grams_per_cup,
    resolve, resolve_line, version, Kind, Resolution, Via,
};

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

fn fdc_id_or_none(item: &str) -> Option<u32> {
    match resolve(item) {
        Resolution::Entry { entry, .. } => entry.fdc_id,
        _ => None,
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
        "Japanese soy sauce",
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
    // Baking staples inherit the linked USDA food's density.
    assert_close(density("all-purpose flour"), 125.0);
    assert_close(density("butter"), 227.0);
    // USDA cup rows the imported average misses: brown sugar's "cup packed"
    // and heavy cream's fluid (not whipped) cup.
    assert_close(density("brown sugar"), 220.0);
    assert_close(density("heavy cream"), 238.0);
    // Flours USDA doesn't weigh by the cup the way recipes measure them.
    assert_close(density("almond flour"), 96.0);
    assert_close(density("cake flour"), 112.0);
    assert_close(density("coconut flour"), 112.0);
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
            matches!(resolve(item), Resolution::Ambiguous(_)),
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
fn version_is_stable() {
    assert!(version().starts_with("catalog-v3-sr2018-"));
    assert_eq!(version(), version());
}

#[test]
fn looser_spellings_resolve_to_the_same_food() {
    let same = |written: &str, canonical: &str| {
        assert_eq!(
            fdc_id(written),
            fdc_id(canonical),
            "{written:?} should match {canonical:?}"
        );
    };
    let via = |item| match resolve(item) {
        Resolution::Entry { via, .. } => via,
        other => panic!("{item:?} did not resolve: {other:?}"),
    };

    // Trailing clauses are dropped one at a time.
    same("kosher salt, presumably diamond", "kosher salt");
    same(
        "diamond crystal kosher salt; for table salt, use half as much by volume",
        "kosher salt",
    );
    assert_eq!(via("kosher salt, presumably diamond"), Via::Clause);
    // Leading size/preparation words, including comma-separated runs.
    same("freshly ground black pepper", "black pepper");
    same("large eggs", "eggs");
    same("chopped fresh cilantro", "cilantro");
    assert_eq!(via("large eggs"), Via::LeadingModifiers);
    // es/ies plurals.
    same("strawberry", "strawberries");
    same("radish", "radishes");
    assert_eq!(via("strawberry"), Via::Plural);
}

#[test]
fn trimming_never_settles_for_a_non_food() {
    // Cutting at the comma would leave "boneless", which is not a food.
    assert!(!matches!(
        resolve("boneless, skinless mystery thighs"),
        Resolution::Entry { .. }
    ));
    // Words that change the food are not stripped.
    for (written, not) in [("ground beef", "beef"), ("dried apricots", "apricots")] {
        if let (Resolution::Entry { entry: a, .. }, Resolution::Entry { entry: b, .. }) =
            (resolve(written), resolve(not))
        {
            assert_ne!(a.id, b.id, "{written:?} must not collapse to {not:?}");
        }
    }
    // A stripped name that is ambiguous stays ambiguous.
    assert!(matches!(
        resolve("shredded cheese"),
        Resolution::Ambiguous(_)
    ));
}

#[test]
fn fresh_herbs_never_become_dried_spices() {
    // Stripping one word at a time finds the more specific name first.
    assert_eq!(fdc_id("grated fresh ginger"), fdc_id("fresh ginger"));
    assert_eq!(fdc_id("minced ginger"), fdc_id("fresh ginger"));
    // Dropping "fresh" or "chopped" must not land on the dried/ground spice:
    // these either resolve to the fresh herb or stay unknown.
    for item in ["fresh rosemary", "chopped sage", "fresh oregano"] {
        if let Resolution::Entry { entry, .. } = resolve(item) {
            assert!(
                !entry.id.starts_with("spices, ") && !entry.id.contains("dried"),
                "{item:?} must not match a dried spice, got {:?}",
                entry.id
            );
        }
    }
    assert_eq!(fdc_id("fresh rosemary"), fdc_id("rosemary, fresh"));
    // ...unless the name says so, or a curated alias covers the phrase.
    assert_eq!(fdc_id("freshly grated nutmeg"), fdc_id("ground nutmeg"));
    assert_eq!(fdc_id("chopped fresh thyme"), fdc_id("thyme, fresh"));
}

#[test]
fn density_approximations_keep_the_real_food_for_calories() {
    // USDA has no volume portion for dried thyme, so its density borrows fresh
    // thyme's, but its calories must come from dried thyme.
    assert_eq!(fdc_id("dried thyme"), Some(170938));
    assert_close(density("dried thyme"), 38.4);
    assert_eq!(fdc_id("thyme"), Some(173470));
    assert_eq!(fdc_id("greek yogurt"), Some(170894));
    assert_close(density("greek yogurt"), 245.0);
    // USDA has these foods, so they are not stood in for by chicken broth.
    assert_eq!(fdc_id("beef broth"), Some(171538));
    assert_close(density("beef broth"), 240.0);
    assert_eq!(fdc_id("vegetable broth"), Some(171583));
    assert_close(density("vegetable broth"), 221.0);
}

#[test]
fn headers_and_serving_notes_are_not_food() {
    for item in [
        "and for the other side of the world:",
        "For the sauce:",
        "to serve",
    ] {
        assert!(
            matches!(resolve(item), Resolution::NotFood),
            "{item:?} should not be a food"
        );
    }
    // Trimming a clause never settles for a non-food.
    assert!(!matches!(
        resolve("to serve, lime wedges"),
        Resolution::NotFood
    ));
}

#[test]
fn products_carry_only_a_shopping_category() {
    let Resolution::Entry { entry, .. } = resolve("Parchment Paper") else {
        panic!("parchment paper should resolve");
    };
    assert_eq!(entry.kind, Kind::Product);
    assert_eq!(entry.fdc_id, None);
    assert_eq!(grams_per_cup("parchment paper"), None);
    assert_eq!(
        category("wooden skewers, soaked in cold water"),
        Some("Household")
    );
}

#[test]
fn foods_and_ambiguous_names_carry_curated_categories() {
    // A USDA food, reached through a curated entry and through an alias.
    assert_eq!(category("butter"), Some("Dairy & Eggs"));
    assert_eq!(category("canned pumpkin"), Some("Canned Goods"));
    // An ambiguous name has its own category but still no food.
    assert!(matches!(resolve("cheese"), Resolution::Ambiguous(_)));
    assert_eq!(category("cheese"), Some("Cheese"));
    // A compound line takes its first food's category.
    assert_eq!(category("salt and pepper"), Some("Spices & Seasonings"));
    assert_eq!(category("moon dust"), None);
}

#[test]
fn compound_lines_list_every_food() {
    for (item, count) in [
        ("salt and pepper", 2),
        ("kosher salt and freshly ground black pepper", 2),
        ("salt & pepper to taste", 2),
        ("olive oil and balsamic vinegar", 2),
    ] {
        match resolve(item) {
            Resolution::Compound(entries) => assert_eq!(entries.len(), count, "{item}"),
            other => panic!("{item:?} should be a compound, got {other:?}"),
        }
        assert_eq!(
            grams_per_cup(item),
            None,
            "{item}: mixture ratio is unknown"
        );
    }
    // A single food named with "and" is not split.
    assert!(matches!(resolve("half and half"), Resolution::Entry { .. }));
    assert!(matches!(resolve("half & half"), Resolution::Entry { .. }));
    // Every part must be a specific food.
    assert!(!matches!(
        resolve("macaroni and cheese"),
        Resolution::Compound(_)
    ));
}

#[test]
fn negligibility_attributes_come_from_the_data() {
    let entry = |item| match resolve(item) {
        Resolution::Entry { entry, .. } => entry,
        other => panic!("{item:?} did not resolve: {other:?}"),
    };
    for item in ["salt", "fine sea salt", "water", "baking soda"] {
        assert!(entry(item).zero_calorie, "{item} has no calories");
    }
    for item in ["black pepper", "ground cumin", "paprika"] {
        let entry = entry(item);
        assert!(
            entry.trace_ok && !entry.zero_calorie,
            "{item} is a trace spice"
        );
    }
    assert!(!entry("butter").trace_ok);
}

#[test]
fn dissimilar_alternatives_and_bare_herbs() {
    // Alternatives count as the first-listed one that names a food (owner
    // decision 2026-10-02), even when they differ a lot in calories; the
    // estimate says which it assumed.
    for (item, assumed) in [
        ("heavy cream or milk", "heavy cream"),
        ("milk or water", "milk"),
        ("sour cream or plain yogurt", "sour cream"),
        (
            "melted, unsalted butter, olive oil, coconut oil, or ghee",
            "unsalted butter",
        ),
        // "country bread" isn't a catalog name, but it is bread.
        ("country or sourdough bread", "bread"),
        ("vegetable, canola, or peanut oil", "vegetable oil"),
        ("cheddar and/or monterey jack cheese", "cheddar cheese"),
    ] {
        assert!(
            matches!(
                resolve(item),
                Resolution::Entry {
                    via: Via::Alternative,
                    ..
                }
            ),
            "{item:?}"
        );
        assert_eq!(
            chosen_alternative(item).as_deref(),
            Some(assumed),
            "{item:?}"
        );
        assert_eq!(fdc_id(item), fdc_id(assumed), "{item:?}");
    }
    // A single word is usually an adjective: it takes the list's noun first.
    for (item, assumed) in [
        ("corn or flour tortillas", "corn tortillas"),
        ("lemon or lime juice", "lemon juice"),
        ("sherry or red wine vinegar", "sherry vinegar"),
        ("apple, grape, or cranberry juice", "apple juice"),
        // Joined prep words and example markers don't push the count onto a
        // later option.
        ("cooked and cooled white or brown rice", "cooked white rice"),
        (
            "soft cheese, like cream cheese, brie, or boursin",
            "cream cheese",
        ),
        ("fresh lemon or lime juice", "fresh lemon juice"),
        // "X or Y noun" is X noun past a one-word or parallel qualifier...
        ("white wine or champagne vinegar", "white wine vinegar"),
        ("white wine or other mild vinegar", "white wine vinegar"),
        ("red wine or white wine vinegar", "red wine vinegar"),
        // ...and X as written past a distinct one, or when X noun is no food.
        ("white wine or plain white vinegar", "white wine"),
        ("white wine or white balsamic vinegar", "white wine"),
        ("rice wine or apple cider vinegar", "rice wine"),
        ("sour cream or plain greek yogurt", "sour cream"),
        (
            "berries: sliced strawberries, blackberries, or raspberries",
            "sliced strawberries",
        ),
        // An article is not an amount.
        ("peanut or a vegetable oil", "peanut oil"),
        // A prep word ("baking") doesn't push the count onto a later option.
        ("baking walnuts or pecans", "baking walnuts"),
        // A measured last alternative lends no noun.
        ("vanilla or half a vanilla bean", "vanilla"),
        (
            "oil canola, olive, or other high-heat cooking oil",
            "canola oil",
        ),
    ] {
        assert_eq!(
            chosen_alternative(item).as_deref(),
            Some(assumed),
            "{item:?}"
        );
        assert_eq!(fdc_id(item), fdc_id(assumed), "{item:?}");
    }
    assert_eq!(fdc_id("baking walnuts or pecans"), fdc_id("walnuts"));
    assert_eq!(
        fdc_id("raw or toasted, unsalted pecan halves"),
        fdc_id("pecans")
    );
    // A trailing synonym ("or stock") doesn't hide the earlier options' noun.
    let broth = "low-sodium chicken, vegetable or seafood broth or stock";
    assert!(
        chosen_alternative(broth)
            .unwrap()
            .ends_with("chicken broth"),
        "{broth:?}"
    );
    assert_eq!(fdc_id(broth), fdc_id("chicken broth"));
    // A first option that is a known food under an unrecognized word counts
    // as that food, never as a later option.
    for (item, assumed) in [
        (
            "small-curd cottage cheese, sour cream, or yogurt",
            "cottage cheese",
        ),
        ("local honey or maple syrup", "honey"),
        ("mixed cherry or grape tomatoes", "cherry tomatoes"),
    ] {
        assert_eq!(
            chosen_alternative(item).as_deref(),
            Some(assumed),
            "{item:?}"
        );
        assert_eq!(fdc_id(item), fdc_id(assumed), "{item:?}");
    }
    assert_eq!(
        fdc_id("full-fat cottage cheese, sour cream, or yogurt"),
        fdc_id("cottage cheese")
    );
    // A first option naming no known food is passed over.
    assert_eq!(
        chosen_alternative("quark or cream cheese").as_deref(),
        Some("cream cheese")
    );
    // Only a one-word option gives way to the whole borrowed noun: not to a
    // fragment of it, and not when that drops the option's own words.
    for item in [
        "sesame or poppy seeds to sprinkle",
        "beni shoga or pickled ginger",
    ] {
        assert_ne!(fdc_id_or_none(item), fdc_id("sprinkles"), "{item:?}");
        assert_ne!(fdc_id_or_none(item), fdc_id("ginger"), "{item:?}");
    }
    // ...unless the words it drops change the food: cooked lentils are not
    // dry lentils, and cannellini beans aren't lentils at all.
    let lentils = "cooked red lentils or cannellini beans";
    assert!(
        matches!(resolve(lentils), Resolution::Unresolved),
        "{lentils:?}"
    );
    assert_eq!(chosen_alternative(lentils), None);
    // A curated alias for an alternatives name is labeled with the listed
    // alternative it counts as.
    assert_eq!(
        chosen_alternative("butter or margarine").as_deref(),
        Some("butter")
    );
    // "cooled" is dropped only when joined to "cooked": alone it still
    // implies cooked rice, never the raw entry.
    assert_ne!(fdc_id_or_none("cooled white rice"), fdc_id("white rice"));
    // An alias whose chosen spelling names nothing alone still says what it
    // assumed.
    assert_eq!(
        chosen_alternative("bundle lacinato kale, swiss chard or spinach").as_deref(),
        Some("bundle lacinato kale")
    );
    assert_eq!(
        fdc_id("chopped fresh collard greens or kale"),
        fdc_id("collards")
    );
    // A footnote marker doesn't push the count onto a later option.
    assert_eq!(
        fdc_id("dried bread crumbs*** or panko"),
        fdc_id("dried bread crumbs")
    );
    // A recipe's "maple" is maple syrup, not a cue to count the honey.
    assert_eq!(fdc_id("maple or honey"), fdc_id("maple syrup"));
    // A name that resolves on its own chose nothing.
    assert_eq!(chosen_alternative("heavy cream"), None);
    // Bare herbs follow how recipes use them.
    assert_eq!(fdc_id("rosemary"), fdc_id("rosemary, fresh"));
    assert_eq!(fdc_id("ginger"), fdc_id("fresh ginger"));
    assert!(matches!(resolve("sage"), Resolution::Ambiguous(_)));
    assert!(matches!(resolve("oregano"), Resolution::Entry { .. }));
}

#[test]
fn salmon_defaults_to_farmed_unless_named() {
    let kcal = |item| food(fdc_id(item).unwrap()).unwrap().kcal_per_100g.unwrap();
    assert_eq!(fdc_id("salmon"), fdc_id("fish, salmon, atlantic, farmed"));
    assert!(kcal("wild salmon") < kcal("salmon"));
    assert_eq!(fdc_id("sockeye salmon"), fdc_id("fish, salmon, sockeye"));
}

#[test]
fn a_peeled_note_selects_the_peeled_food() {
    let line_fdc = |item, note| match resolve_line(item, note) {
        Resolution::Entry { entry, .. } => entry.fdc_id,
        other => panic!("{item:?} did not resolve: {other:?}"),
    };
    let peeled = fdc_id("apples, raw, without skin");
    assert_ne!(fdc_id("apples"), peeled);
    for note in [
        "peeled",
        "peeled and cored",
        "peeled, seeded and coarsely chopped",
        "peeled, but not cored",
        "thawed if frozen, peeled",
    ] {
        assert_eq!(line_fdc("apples", Some(note)), peeled, "{note}");
        assert_eq!(line_fdc("tart apple", Some(note)), peeled, "{note}");
    }
    for note in [
        None,
        Some("unpeeled"),
        Some("no need to peel, cored"),
        Some("not peeled"),
        Some("should not be peeled"),
        Some("peeled only if desired"),
        Some("don't bother, peeled or unpeeled"),
    ] {
        assert_eq!(line_fdc("apples", note), fdc_id("apples"), "{note:?}");
    }
    // USDA weighs peeled apples by the cup of slices.
    assert_eq!(
        line_grams_per_cup("apples", Some("peeled and cored")),
        Some(110.0)
    );
    // USDA has no peeled granny smith, so the apple stays as written.
    assert_eq!(
        line_fdc("granny smith apple", Some("peeled")),
        fdc_id("granny smith apple")
    );
    assert_eq!(
        line_fdc("cucumber", Some("peeled and sliced")),
        fdc_id("cucumber, peeled, raw")
    );
    // Draining is ignored: the can is measured with its liquid.
    assert_eq!(
        line_fdc("chickpeas", Some("drained and rinsed")),
        fdc_id("chickpeas")
    );
}

#[test]
fn a_fat_note_selects_the_ground_meat_blend() {
    let line_fdc = |item, note| match resolve_line(item, note) {
        Resolution::Entry { entry, .. } => entry.fdc_id,
        other => panic!("{item:?} did not resolve: {other:?}"),
    };
    let blend = |lean: u32| {
        fdc_id(&format!(
            "beef, ground, {lean}% lean meat / {}% fat, raw",
            100 - lean
        ))
    };
    assert_eq!(fdc_id("ground beef"), blend(80));
    assert_eq!(line_fdc("ground beef", Some("at least 15% fat")), blend(85));
    assert_eq!(line_fdc("ground beef", Some("80% lean/20% fat")), blend(80));
    assert_eq!(line_fdc("ground beef", Some("93% lean")), blend(93));
    assert_eq!(line_fdc("ground beef", Some("90/10")), blend(90));
    // No USDA blend has 17% fat, and a ratio must add up to 100.
    assert_eq!(line_fdc("ground beef", Some("17% fat")), blend(80));
    assert_eq!(line_fdc("ground beef", Some("1/2 pound")), blend(80));
}

#[test]
fn a_cooked_note_selects_the_cooked_food() {
    let line_fdc = |item, note| match resolve_line(item, note) {
        Resolution::Entry { entry, .. } => entry.fdc_id,
        other => panic!("{item:?} did not resolve: {other:?}"),
    };
    // "brown rice, cooked" parses to item "brown rice" with note "cooked".
    assert_eq!(
        line_fdc("brown rice", Some("cooked")),
        fdc_id("cooked brown rice")
    );
    assert_ne!(line_fdc("brown rice", Some("cooked")), fdc_id("brown rice"));
    assert_eq!(
        line_fdc("brown rice", Some("uncooked")),
        fdc_id("brown rice")
    );
    assert_eq!(
        line_fdc("brown rice", Some("leftover cooked")),
        fdc_id("cooked brown rice")
    );
    assert_eq!(
        line_fdc("brown rice", Some("cooked (about 1 cup uncooked)")),
        fdc_id("cooked brown rice")
    );
    // A cooking instruction applies after measuring, so the raw food is measured.
    for note in [
        "cooked and crumbled",
        "cooked al dente",
        "cooked until crisp",
        "cooked, drained, and cut into small pieces",
    ] {
        assert_eq!(line_fdc("bacon", Some(note)), fdc_id("bacon"), "{note}");
    }
    assert_eq!(line_fdc("brown rice", None), fdc_id("brown rice"));
    // With no cooked form in the catalog, a cooked measure stays unresolved
    // rather than being charged as the raw food.
    assert!(matches!(
        resolve_line("moon dust", Some("cooked")),
        Resolution::Unresolved
    ));
    // With no cooked form in the catalog, a cooked measure stays unresolved
    // rather than being charged as the raw food...
    assert!(matches!(
        resolve_line("granulated sugar", Some("cooked")),
        Resolution::Unresolved
    ));
    // ...unless the item already names a cooked food.
    assert_eq!(
        line_fdc("cooked brown rice", Some("cooked")),
        fdc_id("cooked brown rice")
    );
    // An uncooked entry doesn't name a cooked food.
    assert!(matches!(
        resolve_line("dried apricots", Some("cooked")),
        Resolution::Unresolved
    ));
}

#[test]
fn a_beverage_note_selects_the_beverage_milk() {
    let line_fdc = |item, note| match resolve_line(item, note) {
        Resolution::Entry { entry, .. } => entry.fdc_id,
        other => panic!("{item:?} did not resolve: {other:?}"),
    };
    let kcal = |fdc: Option<u32>| food(fdc.unwrap()).unwrap().kcal_per_100g.unwrap();
    let canned = fdc_id("canned coconut milk");
    let beverage = fdc_id("coconut milk beverage");
    assert!(kcal(beverage) * 5.0 < kcal(canned));
    assert_eq!(line_fdc("coconut milk", None), canned);
    for note in [
        "refrigerated kind, such as Silk",
        "from a carton",
        "beverage",
    ] {
        assert_eq!(line_fdc("coconut milk", Some(note)), beverage, "{note}");
    }
    assert_eq!(line_fdc("unsweetened coconut milk", None), canned);
    assert_eq!(
        line_fdc("unsweetened coconut milk", Some("from a carton")),
        beverage
    );
    // A chilled can (for whipping) is still canned, and brands aren't signals.
    for note in [
        "refrigerated",
        "refrigerated overnight",
        "not refrigerated, divided",
        "chilled overnight",
        "such as Silk",
        "so delicious",
    ] {
        assert_eq!(line_fdc("coconut milk", Some(note)), canned, "{note}");
    }
    // Milks that already default to the beverage keep it.
    assert_eq!(
        line_fdc("almond milk", Some("carton")),
        line_fdc("almond milk", None)
    );
}

#[test]
fn bone_in_weights_are_not_priced_as_meat() {
    for item in ["whole chicken", "bone-in chicken thighs", "chicken wings"] {
        assert!(
            matches!(resolve(item), Resolution::Ambiguous(_)),
            "{item:?} includes bone weight"
        );
    }
    assert!(matches!(
        resolve("boneless skinless chicken thighs"),
        Resolution::Entry { .. }
    ));
}

#[test]
fn counted_pieces_use_usda_portions() {
    let piece = |item, unit| match resolve(item) {
        Resolution::Entry { entry, .. } => grams_per_piece(entry, unit),
        other => panic!("{item:?} did not resolve: {other:?}"),
    };
    // Egg sizes, with a bare count meaning large (a curated override).
    assert_eq!(piece("eggs", None), Some(50.0));
    for (unit, grams) in [
        ("small", 38.0),
        ("medium", 44.0),
        ("large", 50.0),
        ("extra-large", 56.0),
        ("Extra Large", 56.0),
        ("jumbo", 63.0),
    ] {
        assert_eq!(piece("eggs", Some(unit)), Some(grams), "{unit}");
    }
    assert_eq!(piece("garlic", Some("cloves")), Some(3.0));
    assert_eq!(piece("garlic", None), Some(3.0));
    // A cited override adds a head (Canadian Nutrient File: 1 bulb = 24 g).
    assert_eq!(piece("garlic", Some("head")), Some(24.0));
    assert_eq!(piece("onion", None), Some(110.0));
    assert_eq!(piece("celery", Some("stalks")), Some(40.0));
    assert_eq!(piece("celery", Some("large")), Some(64.0));
    assert_eq!(piece("butter", Some("stick")), Some(113.0));
    assert_eq!(piece("lemon", None), Some(58.0));
    assert_eq!(piece("jalapeño", None), Some(14.0));
    // The importer never makes a slice the default, but bacon is counted by
    // the strip, so a curated override does.
    assert_eq!(piece("bacon", None), Some(28.0));
    assert_eq!(piece("bacon", Some("slices")), Some(28.0));
    assert_eq!(piece("bread", None), None);
    // Spices have no piece weight.
    assert_eq!(piece("bay leaves", None), None);
}

#[test]
fn fresh_herbs_are_trace_foods() {
    for item in [
        "thyme", "rosemary", "basil", "parsley", "cilantro", "dill", "mint",
    ] {
        match resolve(item) {
            Resolution::Entry { entry, .. } => assert!(entry.trace_ok, "{item}"),
            other => panic!("{item:?} did not resolve: {other:?}"),
        }
    }
}

#[test]
fn curated_names_are_what_the_parser_produces() {
    // Aliases keyed on text the parser splits off ("about 7 cloves garlic",
    // "chickpeas, rinsed") never match a newly parsed line. `make
    // catalog-clean-aliases` removes or re-keys them; only conflicts, which
    // need a person to decide, may remain.
    let (_, changes) =
        ramekin_core::catalog::clean_curated(ramekin_core::catalog::CURATED_JSON, &typed_names());
    let stale: Vec<_> = changes
        .iter()
        .filter(|change| {
            !matches!(
                change,
                ramekin_core::catalog::CuratedChange::Conflict { .. }
            )
        })
        .collect();
    assert!(
        stale.is_empty(),
        "run make catalog-clean-aliases: {stale:#?}"
    );
}

#[test]
fn parsed_name_never_cuts_into_a_name() {
    // A leading number that is part of the name is never taken as an amount.
    assert_eq!(
        ramekin_core::catalog::parsed_name("85% lean ground beef"),
        "85% lean ground beef"
    );
    assert_eq!(
        ramekin_core::catalog::parsed_name("5- to 6-inch cubanelle chiles"),
        "5- to 6-inch cubanelle chiles"
    );
    assert_eq!(
        ramekin_core::catalog::parsed_name("about 7 cloves garlic, minced"),
        "garlic"
    );
    // A leading size or count a measured line keeps ("1 cup medium grain
    // rice") is part of the stored name, though the bare name reads it as one.
    for name in [
        "medium grain rice",
        "small new potatoes",
        "packed dark-brown sugar",
        "1-inch bread cubes",
        "two us sticks unsalted butter",
    ] {
        assert_eq!(ramekin_core::catalog::parsed_name(name), name);
    }
    assert_eq!(
        ramekin_core::catalog::parsed_name("chickpeas, rinsed"),
        "chickpeas"
    );
}

fn entry_kcal(item: &str) -> Option<f64> {
    match resolve(item) {
        Resolution::Entry { entry, .. } => entry.kcal_per_100g,
        other => panic!("{item:?} did not resolve: {other:?}"),
    }
}

#[test]
fn secondary_sources_supply_foods_sr_legacy_lacks() {
    // An FNDDS food, linked from a curated entry: its calories and density
    // come from the generated fndds.json.
    let guacamole = food(2709307).expect("FNDDS food is loaded");
    assert_eq!(guacamole.description, "guacamole, nfs");
    assert_eq!(fdc_id("guacamole"), Some(2709307));
    assert_eq!(entry_kcal("guacamole"), guacamole.kcal_per_100g);
    assert_eq!(grams_per_cup("guacamole"), guacamole.grams_per_cup);
    // FNDDS descriptions are not names on their own: no curated entry links
    // FNDDS "milk, human", so the resolver never lands on it.
    assert_eq!(food(2705383).unwrap().description, "milk, human");
    if let Resolution::Entry { entry, .. } = resolve("milk, human") {
        assert_ne!(entry.fdc_id, Some(2705383));
    }
    // A hand-curated food with cited calories and no USDA food.
    assert_eq!(fdc_id("garam masala"), None);
    assert_close(entry_kcal("garam masala").unwrap(), 300.0);
    assert_close(density("garam masala"), 160.0);
    assert!(version().contains("fndds2024"));
}

#[test]
fn dish_names_are_never_not_food() {
    // "1 lb meatballs" is real food: marking a dish name not-food would drop
    // its calories silently, so it must stay unknown (or resolve) instead.
    for name in [
        "cake",
        "soup",
        "stew",
        "meatballs",
        "pancakes",
        "frosting",
        "pastry",
        "tacos",
        "wontons",
        "burritos",
        "filling",
        "marinade",
        "slaw",
        "cupcakes",
        "cookie",
    ] {
        assert!(
            !matches!(resolve(name), Resolution::NotFood),
            "{name:?} must not be not-food"
        );
    }
}

#[test]
fn curated_pieces_fill_counts_usda_lacks() {
    let piece = |item, unit| match resolve(item) {
        Resolution::Entry { entry, .. } => grams_per_piece(entry, unit),
        other => panic!("{item:?} did not resolve: {other:?}"),
    };
    // Overrides on SR Legacy foods: new cited pieces, and defaults for a
    // bare count among the food's own pieces.
    assert_eq!(piece("corn tortillas", None), Some(24.0));
    assert_eq!(piece("corn tortillas", Some("small")), Some(18.0));
    assert_eq!(piece("anchovy fillets", None), Some(4.0));
    assert_eq!(piece("ears of corn", None), Some(143.0));
    assert_eq!(piece("parsnips", None), Some(84.56));
    assert_eq!(piece("ham", Some("slices")), Some(28.0));
    // Entry pieces replace the shared food's: buns differ by name.
    assert_eq!(piece("hamburger buns", None), Some(57.0));
    assert_eq!(piece("hot dog buns", None), Some(44.0));
    assert_eq!(piece("baguette", None), Some(324.0));
    assert_eq!(piece("lasagna noodles", None), Some(17.0));
    // FNDDS-linked entries carry their own pieces.
    assert_eq!(piece("sheets nori", None), Some(2.5));
    assert_eq!(piece("prosciutto", Some("slices")), Some(9.2));
    assert_eq!(piece("lettuce", Some("head")), Some(539.0));
    // Bespoke pieces (bespoke.json): a shallot is medium unless sized.
    assert_eq!(piece("shallots", None), Some(28.0));
    assert_eq!(piece("shallot", Some("large")), Some(42.0));
    assert_eq!(piece("ginger", Some("1-inch piece")), Some(7.4));
    assert_eq!(piece("scallions", Some("bunch")), Some(105.0));
    assert_eq!(piece("broccoli", Some("head")), Some(255.0));
    assert_eq!(piece("active dry yeast", Some("packets")), Some(7.0));
    // Standard US can sizes for a bare "can".
    assert_eq!(piece("black beans", Some("can")), Some(425.0));
    assert_eq!(piece("diced tomatoes", Some("cans")), Some(411.0));
    assert_eq!(piece("tuna", Some("can")), Some(113.0));
    // Bespoke pieces add to, never replace, published ones.
    assert_eq!(piece("broccoli", Some("bunch")), Some(608.0));
    // A bunch has a weight, but a bare count of kale still doesn't.
    assert_eq!(piece("kale", None), None);
    // "Juice of 1 lemon" counts lemons of juice or zest.
    assert_eq!(piece("lemon juice", Some("lemon")), Some(48.0));
    assert_eq!(piece("lime juice", Some("limes")), Some(44.0));
    assert_eq!(piece("orange juice", Some("orange")), Some(86.0));
    assert_eq!(piece("lemon zest", Some("lemons")), Some(6.0));
    assert_eq!(piece("lime zest", Some("lime")), Some(3.0));
    assert_eq!(piece("orange zest", Some("orange")), Some(7.5));
    // A sized piece with no exact portion is the piece.
    assert_eq!(piece("lemon juice", Some("large lemon")), Some(48.0));
    assert_eq!(piece("orange zest", Some("extra-large oranges")), Some(7.5));
}

#[test]
fn name_specific_pieces_replace_the_foods() {
    let piece = |item, unit| match resolve(item) {
        Resolution::Entry { entry, .. } => grams_per_piece(entry, unit),
        other => panic!("{item:?} did not resolve: {other:?}"),
    };
    // Generic French bread has a 139 g slice; a baguette's isn't known.
    assert_eq!(piece("crusty bread", Some("slice")), Some(139.0));
    assert_eq!(piece("baguette", Some("slices")), None);
    // A can of corn is canned corn's, not raw corn's.
    assert_eq!(piece("canned corn", Some("can")), Some(432.0));
    assert_eq!(piece("corn", Some("can")), None);
}

#[test]
fn clean_curated_drops_the_category_of_a_removed_name() {
    // Neither name resolves, so the key goes; a category left on it would
    // name nothing and the catalog would refuse to load.
    let json = r#"{"aliases": {"zzfood, chopped": "zzfood"}, "not_food": {}, "categories": {"zzfood, chopped": "Produce", "zzother": "Produce"}}"#;
    let (cleaned, changes) =
        ramekin_core::catalog::clean_curated(json, &std::collections::BTreeSet::new());
    assert_eq!(
        changes,
        vec![ramekin_core::catalog::CuratedChange::Remove {
            section: "aliases",
            name: "zzfood, chopped".to_string(),
        }]
    );
    let cleaned: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
    assert_eq!(
        cleaned["categories"],
        serde_json::json!({"zzother": "Produce"})
    );
}

#[test]
fn clean_curated_keeps_names_people_type() {
    // A hand-typed shopping-list item is matched as typed, never parsed.
    let json = r#"{"aliases": {"zzfood, chopped": "zzfood"}, "not_food": {}, "categories": {}}"#;
    let typed = std::collections::BTreeSet::from(["zzfood, chopped".to_string()]);
    let (_, changes) = ramekin_core::catalog::clean_curated(json, &typed);
    assert!(changes.is_empty(), "{changes:?}");
}

/// Normalized hand-typed shopping-list items, which curated keys must keep
/// matching (see `clean_curated`).
fn typed_names() -> std::collections::BTreeSet<String> {
    #[derive(serde::Deserialize)]
    struct ShoppingItem {
        item: String,
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../data/shopping-list-categories.json");
    let items: Vec<ShoppingItem> =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    items
        .iter()
        .map(|item| ramekin_core::catalog::normalize(&item.item))
        .collect()
}
