use anyhow::{Context, Result};
use headless_chrome::Browser;
use ramekin_client::apis::configuration::Configuration;
use ramekin_client::apis::{auth_api, recipes_api};
use ramekin_client::models::{CreateRecipeRequest, Ingredient, SignupRequest, UpdateRecipeRequest};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::task::JoinSet;

use crate::import::upload_photo_with_client;

// Test images from scripts/seed_images/
const TEST_IMAGES: &[(&str, &[u8])] = &[
    ("bread.png", include_bytes!("seed_images/bread.png")),
    ("chicken.png", include_bytes!("seed_images/chicken.png")),
    ("curry.png", include_bytes!("seed_images/curry.png")),
    ("oats.png", include_bytes!("seed_images/oats.png")),
    ("pasta.png", include_bytes!("seed_images/pasta.png")),
    ("pizza.png", include_bytes!("seed_images/pizza.png")),
    ("risotto.png", include_bytes!("seed_images/risotto.png")),
    ("salad.png", include_bytes!("seed_images/salad.png")),
    ("shrimp.png", include_bytes!("seed_images/shrimp.png")),
    ("tacos.png", include_bytes!("seed_images/tacos.png")),
];

struct RecipeTemplate {
    title: &'static str,
    instructions: &'static str,
    ingredients: &'static [(&'static str, &'static str, &'static str)],
    tags: &'static [&'static str],
}

const RECIPE_TEMPLATES: &[RecipeTemplate] = &[
    RecipeTemplate {
        title: "Classic Chocolate Chip Cookies",
        instructions: "Cream butter and sugars. Add eggs and vanilla. Mix in flour, baking soda, and salt. Fold in chocolate chips. Bake at 375°F for 10-12 minutes.",
        ingredients: &[
            ("flour", "2.25", "cups"),
            ("butter", "1", "cup"),
            ("sugar", "0.75", "cup"),
            ("brown sugar", "0.75", "cup"),
            ("eggs", "2", ""),
            ("vanilla", "2", "tsp"),
            ("chocolate chips", "2", "cups"),
        ],
        tags: &["dessert", "cookies", "baking"],
    },
    RecipeTemplate {
        title: "Simple Tomato Pasta",
        instructions: "Cook pasta. Sauté garlic in olive oil. Add tomatoes and basil. Simmer 15 minutes. Toss with pasta.",
        ingredients: &[
            ("pasta", "1", "lb"),
            ("tomatoes", "4", ""),
            ("garlic", "4", "cloves"),
            ("olive oil", "3", "tbsp"),
            ("basil", "0.25", "cup"),
        ],
        tags: &["pasta", "italian", "easy"],
    },
    RecipeTemplate {
        title: "Grilled Chicken Caesar Salad",
        instructions: "Season and grill chicken. Chop romaine. Toss with dressing and croutons. Top with parmesan and sliced chicken.",
        ingredients: &[
            ("chicken breast", "2", "lbs"),
            ("romaine lettuce", "2", "heads"),
            ("caesar dressing", "0.5", "cup"),
            ("croutons", "1", "cup"),
            ("parmesan", "0.5", "cup"),
        ],
        tags: &["salad", "chicken", "healthy"],
    },
    RecipeTemplate {
        title: "Vegetable Stir Fry",
        instructions: "Heat wok. Cook vegetables in order of hardness. Add sauce. Serve over rice.",
        ingredients: &[
            ("broccoli", "2", "cups"),
            ("bell peppers", "2", ""),
            ("carrots", "3", ""),
            ("soy sauce", "3", "tbsp"),
            ("ginger", "1", "tbsp"),
            ("garlic", "3", "cloves"),
        ],
        tags: &["stir-fry", "vegetarian", "asian"],
    },
    RecipeTemplate {
        title: "Banana Bread",
        instructions: "Mash bananas. Mix with melted butter and sugar. Add eggs and vanilla. Fold in flour and baking soda. Bake at 350°F for 60 minutes.",
        ingredients: &[
            ("bananas", "3", ""),
            ("butter", "0.5", "cup"),
            ("sugar", "0.75", "cup"),
            ("eggs", "2", ""),
            ("flour", "1.5", "cups"),
            ("baking soda", "1", "tsp"),
        ],
        tags: &["baking", "breakfast", "dessert"],
    },
];

async fn create_user_and_recipes(
    user_num: usize,
    server: &str,
    ui_url: &str,
    run_id: u64,
) -> Result<(String, String, usize)> {
    // Use deterministic random seed for this user
    let mut rng = ChaCha8Rng::seed_from_u64(user_num as u64);

    let username = format!("loadtest_{}_{:04}", run_id, user_num);
    let password = format!("password_{}", user_num);

    let mut config = Configuration::new();
    config.base_path = server.to_string();

    // Create a shared HTTP client for all uploads for this user
    let http_client = reqwest::Client::new();

    // Sign up
    let signup_response = auth_api::signup(
        &config,
        SignupRequest {
            username: username.clone(),
            password: password.clone(),
        },
    )
    .await
    .context("Failed to sign up")?;

    // Set auth token
    config.bearer_access_token = Some(signup_response.token);

    // Create 50-5000 recipes to test endpoint efficiency with large lists
    let num_recipes = rng.random_range(50..=5000);
    let mut recipe_ids = Vec::new();

    println!(
        "User {} ({}): Creating {} recipes",
        user_num, username, num_recipes
    );

    for i in 0..num_recipes {
        // Progress update every 100 recipes
        if i > 0 && i % 100 == 0 {
            println!("  User {}: Created {}/{} recipes", user_num, i, num_recipes);
        }

        // Pick a recipe template deterministically
        let template_idx = (user_num * 100 + i) % RECIPE_TEMPLATES.len();
        let template = &RECIPE_TEMPLATES[template_idx];

        // Create recipe
        let title = format!("{} (Variation {})", template.title, i + 1);
        let ingredients: Vec<Ingredient> = template
            .ingredients
            .iter()
            .map(|(item, amount, unit)| Ingredient {
                item: item.to_string(),
                amount: Some(Some(amount.to_string())),
                unit: Some(Some(unit.to_string())),
                note: None,
            })
            .collect();

        let recipe_request = CreateRecipeRequest {
            title,
            instructions: template.instructions.to_string(),
            ingredients,
            description: Some(format!("Recipe {} for user {}", i + 1, username)),
            tags: Some(template.tags.iter().map(|s| s.to_string()).collect()),
            photo_ids: None,
            source_name: None,
            source_url: None,
            servings: None,
            prep_time: None,
            cook_time: None,
            total_time: None,
            rating: None,
            difficulty: None,
            nutritional_info: None,
            notes: None,
        };

        let recipe_response = recipes_api::create_recipe(&config, recipe_request)
            .await
            .context("Failed to create recipe")?;
        recipe_ids.push(recipe_response.id);

        // Upload a photo - pick one based on recipe index
        let image_idx = i % TEST_IMAGES.len();
        let (_image_name, image_data) = TEST_IMAGES[image_idx];
        let photo_id = upload_photo_with_client(&config, image_data, &http_client)
            .await
            .context("Failed to upload photo")?;

        // Update recipe to add the photo
        recipes_api::update_recipe(
            &config,
            &recipe_response.id.to_string(),
            UpdateRecipeRequest {
                title: None,
                instructions: None,
                ingredients: None,
                description: None,
                tags: None,
                photo_ids: Some(Some(vec![photo_id])),
                source_name: None,
                source_url: None,
                servings: None,
                prep_time: None,
                cook_time: None,
                total_time: None,
                rating: None,
                difficulty: None,
                nutritional_info: None,
                notes: None,
            },
        )
        .await
        .context("Failed to update recipe with photo")?;
    }

    // Edit each recipe (update instructions)
    for recipe_id in &recipe_ids {
        recipes_api::update_recipe(
            &config,
            &recipe_id.to_string(),
            UpdateRecipeRequest {
                title: None,
                instructions: Some(Some("Updated instructions during load test.".to_string())),
                ingredients: None,
                description: None,
                tags: None,
                photo_ids: None,
                source_name: None,
                source_url: None,
                servings: None,
                prep_time: None,
                cook_time: None,
                total_time: None,
                rating: None,
                difficulty: None,
                nutritional_info: None,
                notes: None,
            },
        )
        .await
        .context("Failed to update recipe")?;
    }

    println!(
        "  User {}: Finished creating {} recipes, now loading pages in browser...",
        user_num, num_recipes
    );

    // Load pages in headless browser to simulate real usage and test frontend performance
    let start = std::time::Instant::now();

    let browser = Browser::default().context("Failed to launch browser")?;
    let tab = browser.new_tab().context("Failed to create tab")?;

    // Navigate to login page
    tab.navigate_to(ui_url)
        .context("Failed to navigate to UI")?;
    tab.wait_for_element("input[type='text']")
        .context("Failed to find username input")?;

    // Log in
    tab.wait_for_element("input[type='text']")
        .context("Failed to find username input")?
        .click()
        .context("Failed to click username input")?
        .type_into(&username)
        .context("Failed to type username")?;
    tab.wait_for_element("input[type='password']")
        .context("Failed to find password input")?
        .click()
        .context("Failed to click password input")?
        .type_into(&password)
        .context("Failed to type password")?;
    tab.wait_for_element("button[type='submit']")
        .context("Failed to find submit button")?
        .click()
        .context("Failed to click submit")?;

    // Wait for cookbook page to load
    std::thread::sleep(std::time::Duration::from_secs(2));
    tab.wait_for_element(".recipe-card")
        .context("Failed to wait for recipe cards")?;

    let cookbook_duration = start.elapsed();

    // Click on first recipe
    let recipe_start = std::time::Instant::now();
    tab.wait_for_element(".recipe-card")
        .context("Failed to find recipe card")?
        .click()
        .context("Failed to click recipe card")?;

    // Wait for recipe page to load
    std::thread::sleep(std::time::Duration::from_secs(1));
    tab.wait_for_element(".recipe-photo")
        .context("Failed to wait for recipe photo")?;

    let recipe_duration = recipe_start.elapsed();

    println!(
        "  User {}: Loaded cookbook ({} recipes) in {:.2}s, recipe page in {:.2}s",
        user_num,
        num_recipes,
        cookbook_duration.as_secs_f64(),
        recipe_duration.as_secs_f64()
    );

    Ok((username, password, num_recipes))
}

pub async fn load_test(
    server: &str,
    ui_url: &str,
    num_users: usize,
    concurrency: usize,
) -> Result<()> {
    // Generate unique run ID from current timestamp
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    println!("Starting load test:");
    println!("  Server: {}", server);
    println!("  UI URL: {}", ui_url);
    println!("  Users: {}", num_users);
    println!("  Concurrency: {}", concurrency);
    println!("  Run ID: {}", run_id);
    println!();

    let successes = Arc::new(AtomicUsize::new(0));
    let failures = Arc::new(AtomicUsize::new(0));
    let user_with_most_recipes = Arc::new(std::sync::Mutex::new((
        String::new(),
        String::new(),
        0usize,
    )));

    let mut tasks = JoinSet::new();

    for user_num in 0..num_users {
        let server = server.to_string();
        let ui_url = ui_url.to_string();
        let successes = successes.clone();
        let failures = failures.clone();
        let user_with_most_recipes = user_with_most_recipes.clone();

        tasks.spawn(async move {
            match create_user_and_recipes(user_num, &server, &ui_url, run_id).await {
                Ok((username, password, num_recipes)) => {
                    let count = successes.fetch_add(1, Ordering::Relaxed) + 1;

                    // Track user with most recipes
                    {
                        let mut max_user = user_with_most_recipes.lock().unwrap();
                        if num_recipes > max_user.2 {
                            *max_user = (username, password, num_recipes);
                        }
                    }

                    if count.is_multiple_of(10) {
                        let total = count + failures.load(Ordering::Relaxed);
                        println!(
                            "Progress: {}/{} users processed ({} success, {} failed)",
                            total,
                            num_users,
                            count,
                            failures.load(Ordering::Relaxed)
                        );
                    }
                }
                Err(e) => {
                    failures.fetch_add(1, Ordering::Relaxed);
                    tracing::error!(user_num, error = %e, "Failed to create user");
                    // Log the full error chain
                    let mut source = e.source();
                    while let Some(err) = source {
                        tracing::error!(cause = %err, "Caused by");
                        source = err.source();
                    }
                }
            }
        });

        // Limit concurrency
        if tasks.len() >= concurrency {
            tasks.join_next().await;
        }
    }

    // Wait for remaining tasks
    while tasks.join_next().await.is_some() {}

    let final_successes = successes.load(Ordering::Relaxed);
    let final_failures = failures.load(Ordering::Relaxed);

    println!();
    println!("Load test complete!");
    println!("  Total users: {}", num_users);
    println!("  Successful: {}", final_successes);
    println!("  Failed: {}", final_failures);

    // Print user with most recipes
    let max_user = user_with_most_recipes.lock().unwrap();
    if max_user.2 > 0 {
        println!();
        println!("User with most recipes ({} recipes):", max_user.2);
        println!("  Username: {}", max_user.0);
        println!("  Password: {}", max_user.1);
    }

    Ok(())
}
