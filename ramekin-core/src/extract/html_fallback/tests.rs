use super::*;

#[test]
fn test_html_fallback_seriouseats_print_page() {
    // Serious Eats `?print` pages ship with no JSON-LD or microdata at all —
    // just the rendered Dotdash Meredith CMS HTML. Title comes from
    // h1.heading__title, ingredients from .structured-ingredients with
    // .structured-ingredients__list-heading group headers interleaved between
    // .structured-ingredients__list-item entries, and instructions from
    // .structured-project__steps li > p.mntl-sc-block-html.
    let html = r#"
            <!doctype html>
            <html>
            <body>
                <h1 class="heading__title">Toad in the Hole</h1>
                <section class="comp section--ingredients section">
                    <div class="comp structured-ingredients">
                        <p class="structured-ingredients__list-heading">For the Yorkshire Pudding Batter:</p>
                        <ul class="structured-ingredients__list">
                            <li class="structured-ingredients__list-item"><p>3 large eggs</p></li>
                            <li class="structured-ingredients__list-item"><p>4 ounces all-purpose flour</p></li>
                        </ul>
                        <p class="structured-ingredients__list-heading">For the Red Onion Gravy</p>
                        <ul class="structured-ingredients__list">
                            <li class="structured-ingredients__list-item"><p>2 tablespoons beef drippings</p></li>
                            <li class="structured-ingredients__list-item"><p>1 large red onion, thinly sliced</p></li>
                        </ul>
                    </div>
                </section>
                <section class="comp section--instructions section">
                    <div class="comp structured-project__steps">
                        <ol class="comp mntl-sc-block-group--OL">
                            <li class="comp mntl-sc-block-group--LI">
                                <p class="comp mntl-sc-block-html"><strong>For the Batter:</strong> Whisk eggs, flour, and milk together. Let rest for 30 minutes.</p>
                            </li>
                            <li class="comp mntl-sc-block-group--LI">
                                <p class="comp mntl-sc-block-html"><strong>For the Gravy:</strong> Melt drippings over medium-low heat. Add onions and cook until lightly caramelized.</p>
                            </li>
                            <li class="comp mntl-sc-block-group--LI">
                                <p class="comp mntl-sc-block-html">Slice into wedges and smother each portion in onion gravy.</p>
                            </li>
                        </ol>
                    </div>
                </section>
            </body>
            </html>
        "#;

    let result = extract_recipe(html, "https://www.seriouseats.com/toad-in-the-hole").unwrap();
    assert_eq!(result.title, "Toad in the Hole");
    let ingredient_lines: Vec<&str> = result.ingredients.lines().collect();
    assert_eq!(ingredient_lines[0], "For the Yorkshire Pudding Batter:");
    assert_eq!(ingredient_lines[1], "3 large eggs");
    assert_eq!(ingredient_lines[2], "4 ounces all-purpose flour");
    assert_eq!(ingredient_lines[3], "For the Red Onion Gravy:");
    assert_eq!(ingredient_lines[4], "2 tablespoons beef drippings");
    assert_eq!(ingredient_lines[5], "1 large red onion, thinly sliced");
    assert!(result.instructions.contains("For the Batter:"));
    assert!(result.instructions.contains("Whisk eggs, flour, and milk"));
    assert!(result.instructions.contains("For the Gravy:"));
    assert!(result.instructions.contains("Slice into wedges"));
}

#[test]
fn test_html_fallback_seriouseats_dedupes_unit_name_quirk() {
    // Dotdash Meredith CMS quirk: when an ingredient like "1 baguette"
    // has no real unit, the page renders both `data-ingredient-unit` and
    // `data-ingredient-name` spans with the same word, producing
    // "1 baguette baguette". Strip the duplicate.
    let html = r#"
            <!doctype html>
            <html>
            <body>
                <h1 class="heading__title">Crusty Bread</h1>
                <div class="comp structured-ingredients">
                    <ul class="structured-ingredients__list">
                        <li class="structured-ingredients__list-item">
                            <p>
                                <span data-ingredient-quantity="true">1</span>
                                <span data-ingredient-unit="true">baguette</span>
                                <span data-ingredient-name="true">baguette</span>
                            </p>
                        </li>
                    </ul>
                </div>
                <div class="comp structured-project__steps">
                    <ol>
                        <li class="comp mntl-sc-block-group--LI">
                            <p class="comp mntl-sc-block-html">Slice the baguette and serve.</p>
                        </li>
                    </ol>
                </div>
            </body>
            </html>
        "#;

    let result = extract_recipe(html, "https://www.seriouseats.com/baguette").unwrap();
    let ingredient_lines: Vec<&str> = result.ingredients.lines().collect();
    assert_eq!(ingredient_lines, vec!["1 baguette"]);
}

#[test]
fn test_html_fallback_seriouseats_no_groups() {
    // Print page with a single ingredient list and no group headings.
    let html = r#"
            <!doctype html>
            <html>
            <body>
                <h1 class="heading__title">Simple Vinaigrette</h1>
                <div class="comp structured-ingredients">
                    <ul class="structured-ingredients__list">
                        <li class="structured-ingredients__list-item"><p>3 tablespoons olive oil</p></li>
                        <li class="structured-ingredients__list-item"><p>1 tablespoon vinegar</p></li>
                    </ul>
                </div>
                <div class="comp structured-project__steps">
                    <ol>
                        <li class="comp mntl-sc-block-group--LI">
                            <p class="comp mntl-sc-block-html">Whisk oil and vinegar together until emulsified.</p>
                        </li>
                    </ol>
                </div>
            </body>
            </html>
        "#;

    let result = extract_recipe(html, "https://www.seriouseats.com/vinaigrette").unwrap();
    assert_eq!(result.title, "Simple Vinaigrette");
    assert!(result.ingredients.contains("3 tablespoons olive oil"));
    assert!(result.ingredients.contains("1 tablespoon vinegar"));
    // No spurious group header should appear.
    assert!(!result.ingredients.contains(":"));
    assert!(result.instructions.contains("Whisk oil and vinegar"));
}

#[test]
fn test_dotdash_visible_ingredients_supplement_jsonld() {
    // Dotdash Meredith (Serious Eats) JSON-LD simplifies combined ingredient
    // rows, dropping quantities the visible page keeps. The visible
    // .structured-ingredients rows should win.
    let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Croquetas de Jamón",
                    "recipeIngredient": [
                        "2 cups (473ml) whole milk",
                        "1 cup all-purpose flour, for dredging"
                    ],
                    "recipeInstructions": "Stir in flour, then dredge and fry."
                }
                </script>
            </head>
            <body>
                <div class="comp structured-ingredients">
                    <ul class="structured-ingredients__list">
                        <li class="structured-ingredients__list-item"><p>2 cups (473 ml) whole milk</p></li>
                        <li class="structured-ingredients__list-item"><p>1/2 cup plus 2 tablespoons all-purpose flour (80 g), plus 1 cup all-purpose flour (for dredging), divided</p></li>
                    </ul>
                </div>
            </body></html>
        "#;

    let result = extract_recipe(html, "https://www.seriouseats.com/croquetas").unwrap();
    let lines: Vec<&str> = result.ingredients.lines().collect();
    assert_eq!(
            lines,
            vec![
                "2 cups (473 ml) whole milk",
                "1/2 cup plus 2 tablespoons all-purpose flour (80 g), plus 1 cup all-purpose flour (for dredging), divided",
            ]
        );
}

#[test]
fn test_dotdash_normalized_visible_ingredients_keep_jsonld() {
    // Some Dotdash pages render nutrition-database normalized rows instead
    // of the author's text ("454 g pork breakfast sausage" for "1 pound
    // (454g) pork breakfast sausage, casings removed"). Those rows sit
    // entirely inside data-ingredient-* spans with no free text outside;
    // keep the JSON-LD version, which has the author's rows.
    let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Biscuits and Gravy",
                    "recipeIngredient": [
                        "1 pound (454g) pork breakfast sausage, casings removed",
                        "Freshly ground black pepper"
                    ],
                    "recipeInstructions": "Brown the sausage and make the gravy."
                }
                </script>
            </head>
            <body>
                <div class="comp structured-ingredients">
                    <ul class="structured-ingredients__list">
                        <li class="structured-ingredients__list-item">
                            <p><span data-ingredient-quantity="true">454</span> <span data-ingredient-unit="true">g</span> <span data-ingredient-name="true">pork breakfast sausage</span></p>
                        </li>
                        <li class="structured-ingredients__list-item">
                            <p><span data-ingredient-quantity="true">1</span> <span data-ingredient-unit="true">tsp, ground</span> <span data-ingredient-name="true">ground black pepper</span></p>
                        </li>
                    </ul>
                </div>
            </body></html>
        "#;

    let result = extract_recipe(html, "https://www.seriouseats.com/biscuits").unwrap();
    let lines: Vec<&str> = result.ingredients.lines().collect();
    assert_eq!(
        lines,
        vec![
            "1 pound (454g) pork breakfast sausage, casings removed",
            "Freshly ground black pepper",
        ]
    );
}

#[test]
fn test_dotdash_visible_ingredients_fewer_rows_keeps_jsonld() {
    // If the rendered page shows fewer ingredient rows than the structured
    // data (e.g. a partially rendered list), keep the JSON-LD version.
    let html = r#"
            <!DOCTYPE html>
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Croquetas de Jamón",
                    "recipeIngredient": [
                        "2 cups (473ml) whole milk",
                        "1 cup all-purpose flour, for dredging"
                    ],
                    "recipeInstructions": "Stir in flour, then dredge and fry."
                }
                </script>
            </head>
            <body>
                <div class="comp structured-ingredients">
                    <ul class="structured-ingredients__list">
                        <li class="structured-ingredients__list-item"><p>2 cups (473 ml) whole milk</p></li>
                    </ul>
                </div>
            </body></html>
        "#;

    let result = extract_recipe(html, "https://www.seriouseats.com/croquetas").unwrap();
    let lines: Vec<&str> = result.ingredients.lines().collect();
    assert_eq!(
        lines,
        vec![
            "2 cups (473ml) whole milk",
            "1 cup all-purpose flour, for dredging",
        ]
    );
}

#[test]
fn test_html_fallback_jetpack_ingredients() {
    // Jetpack recipe with ingredients in .jetpack-recipe-ingredient class
    let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <div itemscope itemtype="https://schema.org/Recipe">
                    <h3 itemprop="name">Test Recipe</h3>
                    <div class="jetpack-recipe-content"></div>
                </div>
                <ul>
                    <li class="jetpack-recipe-ingredient">1 cup flour</li>
                    <li class="jetpack-recipe-ingredient">2 eggs</li>
                </ul>
                <div class="jetpack-recipe-directions">Mix and bake at 350.</div>
            </body>
            </html>
        "#;

    let result = extract_recipe(html, "https://example.com/recipe").unwrap();
    assert_eq!(result.title, "Test Recipe");
    assert!(result.ingredients.contains("1 cup flour"));
    assert!(result.ingredients.contains("2 eggs"));
    assert!(result.instructions.contains("Mix and bake"));
}

#[test]
fn test_html_fallback_title_from_entry_title() {
    // JSON-LD with missing name, title available in h1.entry-title
    let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "recipeIngredient": ["1 cup flour"],
                    "recipeInstructions": "Mix and bake."
                }
                </script>
            </head>
            <body>
                <h1 class="entry-title">My Great Recipe</h1>
            </body>
            </html>
        "#;

    let result = extract_recipe(html, "https://example.com/recipe").unwrap();
    assert_eq!(result.title, "My Great Recipe");
}

#[test]
fn test_html_fallback_with_stats_reports_method() {
    // Verify that extract_recipe_with_stats reports HtmlFallback method
    let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Test Recipe",
                    "recipeIngredient": [],
                    "recipeInstructions": "Mix it."
                }
                </script>
            </head>
            <body>
                <div class="ingredients"><p>1 cup flour<br>2 eggs</p></div>
            </body>
            </html>
        "#;

    let result = extract_recipe_with_stats(html, "https://example.com/recipe").unwrap();
    assert_eq!(result.method_used, ExtractionMethod::HtmlFallback);
    assert_eq!(result.raw_recipe.title, "Test Recipe");
    assert!(result.raw_recipe.ingredients.contains("1 cup flour"));
}

#[test]
fn test_smittenkitchen_post_body_instructions_supplement_empty_jsonld() {
    let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <script type="application/ld+json">
                {
                    "@type": "Recipe",
                    "name": "Slow-Roasted Tomatoes",
                    "recipeIngredient": [
                        "Cherry, grape or small Roma tomatoes",
                        "Whole cloves of garlic, unpeeled",
                        "Olive oil",
                        "Herbs such as thyme or rosemary (optional)"
                    ],
                    "recipeInstructions": []
                }
                </script>
            </head>
            <body>
                <div class="entry-content">
                    <div class="smittenkitchen-print-hide">
                        <p>Narrative intro with photos and old post links.</p>
                    </div>
                    <h3>Slow-Roasted Tomatoes</h3>
                    <ul>
                        <li>Time: 3 hours</li>
                        <li>Print</li>
                    </ul>
                    <p>I know what you're going to say about turning on the oven.</p>
                    <ul>
                        <li>Cherry, grape or small Roma tomatoes</li>
                        <li>Whole cloves of garlic, unpeeled</li>
                        <li>Olive oil</li>
                        <li>Herbs such as thyme or rosemary (optional)</li>
                    </ul>
                    <p>Preheat oven to 225°F. Halve each tomato and arrange on a parchment-lined baking sheet.</p>
                    <p>Bake the tomatoes in the oven for about 3 hours.</p>
                    <p>Either use them right away or let them cool and cover them with olive oil.</p>
                    <div class="sharedaddy">Share this:</div>
                    <p>Comment text should not be included.</p>
                </div>
            </body>
            </html>
        "#;

    let result = extract_recipe_with_stats(
        html,
        "https://smittenkitchen.com/2008/08/slow-roasted-tomatoes/",
    )
    .unwrap();

    assert_eq!(result.method_used, ExtractionMethod::HtmlFallback);
    assert_eq!(result.raw_recipe.title, "Slow-Roasted Tomatoes");
    assert!(result.raw_recipe.instructions.contains("Preheat oven"));
    assert!(result.raw_recipe.instructions.contains("Bake the tomatoes"));
    assert!(result.raw_recipe.instructions.contains("Either use them"));
    assert!(!result
        .raw_recipe
        .instructions
        .contains("turning on the oven"));
    assert!(!result.raw_recipe.instructions.contains("Comment text"));
}

#[test]
fn test_jetpack_ingredient_groups_supplement_microdata() {
    // Jetpack uses <h5> headings inside .jetpack-recipe-ingredients to group
    // ingredients. Microdata extraction only picks up the [itemprop] items.
    let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <div itemscope itemtype="https://schema.org/Recipe">
                    <h3 class="jetpack-recipe-title" itemprop="name">Ginger Meatballs</h3>
                    <div class="jetpack-recipe-content">
                        <div class="jetpack-recipe-ingredients">
                            <h5>Meatballs</h5>
                            <ul>
                                <li class="jetpack-recipe-ingredient" itemprop="recipeIngredient">2 pounds ground pork</li>
                                <li class="jetpack-recipe-ingredient" itemprop="recipeIngredient">2 large eggs</li>
                            </ul>
                            <h5>Broth</h5>
                            <ul>
                                <li class="jetpack-recipe-ingredient" itemprop="recipeIngredient">1 can coconut milk</li>
                                <li class="jetpack-recipe-ingredient" itemprop="recipeIngredient">2 cups chicken stock</li>
                            </ul>
                            <h5>To serve</h5>
                            <ul>
                                <li class="jetpack-recipe-ingredient" itemprop="recipeIngredient">Steamed jasmine rice</li>
                            </ul>
                        </div>
                    </div>
                    <div itemprop="recipeInstructions">Make meatballs and broth.</div>
                </div>
            </body>
            </html>
        "#;

    let result = extract_recipe(html, "https://smittenkitchen.com/recipe").unwrap();
    let lines: Vec<&str> = result.ingredients.lines().collect();
    assert_eq!(lines[0], "Meatballs:");
    assert_eq!(lines[1], "2 pounds ground pork");
    assert_eq!(lines[2], "2 large eggs");
    assert_eq!(lines[3], "Broth:");
    assert_eq!(lines[4], "1 can coconut milk");
    assert_eq!(lines[5], "2 cups chicken stock");
    assert_eq!(lines[6], "To serve:");
    assert_eq!(lines[7], "Steamed jasmine rice");
}

#[test]
fn test_jetpack_ingredient_groups_malformed_h5_inside_ul() {
    // Real smittenkitchen HTML: <p> wrapping a <div> (invalid), and <h5>
    // headers inside <ul> (also invalid). html5ever reparses this.
    let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <div itemscope itemtype="https://schema.org/Recipe">
                    <h3 class="jetpack-recipe-title" itemprop="name">Ginger Meatballs</h3>
                    <p><div class="jetpack-recipe-ingredients"><ul>
                        <h5>Meatballs</h5>
                        <li class="jetpack-recipe-ingredient" itemprop="recipeIngredient">2 pounds ground pork</li>
                        <li class="jetpack-recipe-ingredient" itemprop="recipeIngredient">2 large eggs</li>
                        <h5>Broth</h5>
                        <li class="jetpack-recipe-ingredient" itemprop="recipeIngredient">1 can coconut milk</li>
                    </ul></div></p>
                    <div itemprop="recipeInstructions">Make meatballs and broth.</div>
                </div>
            </body>
            </html>
        "#;

    let result = extract_recipe(html, "https://smittenkitchen.com/recipe").unwrap();
    eprintln!("Ingredients:\n{}", result.ingredients);
    assert!(
        result.ingredients.contains("Meatballs"),
        "should contain Meatballs group header"
    );
    assert!(
        result.ingredients.contains("Broth"),
        "should contain Broth group header"
    );
}
