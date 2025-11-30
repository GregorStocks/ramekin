use anyhow::{Context, Result};
use ramekin_client::apis::configuration::Configuration;
use ramekin_client::apis::{auth_api, recipes_api};
use ramekin_client::models::{CreateRecipeRequest, Ingredient, LoginRequest, SignupRequest};

// Embed PNG images at compile time
const IMAGE_PASTA: &[u8] = include_bytes!("../seed_images/pasta.png");
const IMAGE_CHICKEN: &[u8] = include_bytes!("../seed_images/chicken.png");
const IMAGE_BREAD: &[u8] = include_bytes!("../seed_images/bread.png");
const IMAGE_CURRY: &[u8] = include_bytes!("../seed_images/curry.png");
const IMAGE_TACOS: &[u8] = include_bytes!("../seed_images/tacos.png");
const IMAGE_RISOTTO: &[u8] = include_bytes!("../seed_images/risotto.png");
const IMAGE_SALAD: &[u8] = include_bytes!("../seed_images/salad.png");
const IMAGE_PIZZA: &[u8] = include_bytes!("../seed_images/pizza.png");
const IMAGE_OATS: &[u8] = include_bytes!("../seed_images/oats.png");
const IMAGE_SHRIMP: &[u8] = include_bytes!("../seed_images/shrimp.png");

struct SeedRecipe {
    title: &'static str,
    description: Option<&'static str>,
    instructions: &'static str,
    ingredients: &'static [(
        &'static str,
        &'static str,
        &'static str,
        Option<&'static str>,
    )], // (item, amount, unit, note)
    tags: &'static [&'static str],
    source_name: Option<&'static str>,
    image: &'static [u8],
}

const SAMPLE_RECIPES: &[SeedRecipe] = &[
    SeedRecipe {
        title: "Classic Spaghetti Carbonara",
        description: Some("A rich and creamy Italian pasta dish with eggs, cheese, and pancetta."),
        instructions:
            "1. Bring a large pot of salted water to boil and cook spaghetti until al dente.
2. While pasta cooks, cut pancetta into small cubes and fry until crispy.
3. In a bowl, whisk together eggs, grated Pecorino Romano, and black pepper.
4. When pasta is done, reserve 1 cup pasta water, then drain.
5. Working quickly, add hot pasta to the pancetta pan (off heat).
6. Pour egg mixture over pasta and toss vigorously to create a creamy sauce.
7. Add pasta water as needed to reach desired consistency.
8. Serve immediately with extra cheese and black pepper.",
        ingredients: &[
            ("spaghetti", "400", "g", None),
            ("pancetta or guanciale", "200", "g", None),
            ("eggs", "4", "large", None),
            ("Pecorino Romano", "100", "g", Some("freshly grated")),
            ("black pepper", "2", "tsp", Some("freshly ground")),
            ("salt", "", "", Some("for pasta water")),
        ],
        tags: &["italian", "pasta", "dinner", "quick"],
        source_name: Some("Serious Eats"),
        image: IMAGE_PASTA,
    },
    SeedRecipe {
        title: "Chicken Tikka Masala",
        description: Some(
            "Tender chicken pieces in a creamy, spiced tomato sauce. A British-Indian classic.",
        ),
        instructions:
            "1. Marinate chicken in yogurt, garam masala, cumin, and salt for at least 2 hours.
2. Grill or broil marinated chicken until charred and cooked through.
3. In a large pan, sauté onions until golden, then add garlic and ginger.
4. Add tomato puree, cream, and spices. Simmer for 15 minutes.
5. Cut grilled chicken into bite-sized pieces and add to the sauce.
6. Simmer together for 10 minutes to let flavors meld.
7. Garnish with fresh cilantro and serve with basmati rice or naan.",
        ingredients: &[
            ("chicken thighs", "800", "g", Some("boneless, skinless")),
            ("yogurt", "1", "cup", None),
            ("garam masala", "2", "tbsp", None),
            ("cumin", "1", "tsp", None),
            ("onion", "2", "large", Some("diced")),
            ("garlic", "4", "cloves", Some("minced")),
            ("ginger", "2", "inch", Some("grated")),
            ("tomato puree", "400", "g", None),
            ("heavy cream", "1", "cup", None),
            ("cilantro", "", "", Some("for garnish")),
        ],
        tags: &["indian", "chicken", "dinner", "spicy"],
        source_name: None,
        image: IMAGE_CHICKEN,
    },
    SeedRecipe {
        title: "Banana Bread",
        description: Some(
            "Moist and delicious banana bread, perfect for using up overripe bananas.",
        ),
        instructions: "1. Preheat oven to 350°F (175°C). Grease a 9x5 inch loaf pan.
2. Mash bananas in a large bowl until smooth.
3. Mix in melted butter, then sugar, egg, and vanilla.
4. Stir in baking soda and salt, then fold in flour until just combined.
5. Pour batter into prepared pan.
6. Bake for 55-65 minutes until a toothpick comes out clean.
7. Let cool in pan for 10 minutes, then remove to wire rack.",
        ingredients: &[
            ("ripe bananas", "3", "large", None),
            ("butter", "1/3", "cup", Some("melted")),
            ("sugar", "3/4", "cup", None),
            ("egg", "1", "large", None),
            ("vanilla extract", "1", "tsp", None),
            ("baking soda", "1", "tsp", None),
            ("salt", "1/4", "tsp", None),
            ("all-purpose flour", "1.5", "cups", None),
        ],
        tags: &["baking", "breakfast", "dessert", "easy"],
        source_name: Some("Grandma's Recipe Box"),
        image: IMAGE_BREAD,
    },
    SeedRecipe {
        title: "Thai Green Curry",
        description: Some(
            "Aromatic and creamy Thai curry with vegetables and your choice of protein.",
        ),
        instructions: "1. Heat oil in a wok or large pan over medium-high heat.
2. Add green curry paste and fry for 1 minute until fragrant.
3. Add coconut milk and bring to a simmer.
4. Add chicken (or tofu) and cook until nearly done.
5. Add vegetables and simmer until tender-crisp.
6. Season with fish sauce, palm sugar, and lime juice.
7. Stir in Thai basil leaves just before serving.
8. Serve over jasmine rice.",
        ingredients: &[
            ("green curry paste", "3", "tbsp", None),
            ("coconut milk", "400", "ml", None),
            ("chicken breast", "500", "g", Some("sliced")),
            ("Thai eggplant", "1", "cup", Some("quartered")),
            ("bamboo shoots", "1/2", "cup", None),
            ("fish sauce", "2", "tbsp", None),
            ("palm sugar", "1", "tbsp", None),
            ("Thai basil", "1", "cup", None),
            ("lime", "1", "", Some("juiced")),
        ],
        tags: &["thai", "curry", "dinner", "spicy", "gluten-free"],
        source_name: None,
        image: IMAGE_CURRY,
    },
    SeedRecipe {
        title: "Classic Beef Tacos",
        description: Some("Simple and flavorful ground beef tacos with all the fixings."),
        instructions: "1. Brown ground beef in a skillet over medium-high heat.
2. Drain excess fat, then add taco seasoning and water.
3. Simmer for 5-10 minutes until sauce thickens.
4. Warm taco shells according to package directions.
5. Fill shells with seasoned beef.
6. Top with shredded cheese, lettuce, tomatoes, and sour cream.
7. Squeeze fresh lime juice over tacos before serving.",
        ingredients: &[
            ("ground beef", "1", "lb", None),
            ("taco seasoning", "1", "packet", None),
            ("taco shells", "12", "", None),
            ("cheddar cheese", "1", "cup", Some("shredded")),
            ("lettuce", "2", "cups", Some("shredded")),
            ("tomatoes", "2", "", Some("diced")),
            ("sour cream", "1/2", "cup", None),
            ("lime", "1", "", None),
        ],
        tags: &["mexican", "dinner", "quick", "family-friendly"],
        source_name: None,
        image: IMAGE_TACOS,
    },
    SeedRecipe {
        title: "Mushroom Risotto",
        description: Some("Creamy Italian rice dish with savory mushrooms and Parmesan cheese."),
        instructions: "1. Heat broth in a saucepan and keep warm over low heat.
2. Sauté mushrooms in butter until golden, set aside.
3. In a large pan, sauté shallots in olive oil until soft.
4. Add rice and toast for 2 minutes, stirring constantly.
5. Add wine and stir until absorbed.
6. Add warm broth one ladle at a time, stirring until each is absorbed.
7. Continue for 18-20 minutes until rice is creamy but al dente.
8. Stir in mushrooms, butter, and Parmesan. Season to taste.",
        ingredients: &[
            ("arborio rice", "1.5", "cups", None),
            ("vegetable broth", "6", "cups", None),
            ("mixed mushrooms", "300", "g", Some("sliced")),
            ("shallots", "2", "", Some("minced")),
            ("white wine", "1/2", "cup", None),
            ("butter", "3", "tbsp", None),
            ("Parmesan", "1/2", "cup", Some("grated")),
            ("olive oil", "2", "tbsp", None),
        ],
        tags: &["italian", "vegetarian", "dinner", "comfort-food"],
        source_name: None,
        image: IMAGE_RISOTTO,
    },
    SeedRecipe {
        title: "Greek Salad",
        description: Some("Fresh and vibrant Mediterranean salad with feta cheese and olives."),
        instructions: "1. Cut tomatoes into wedges and cucumber into half-moons.
2. Slice red onion into thin rings.
3. Combine vegetables in a large bowl.
4. Add olives and crumbled feta cheese.
5. Drizzle with olive oil and red wine vinegar.
6. Season with oregano, salt, and pepper.
7. Toss gently and serve immediately.",
        ingredients: &[
            ("tomatoes", "4", "large", None),
            ("cucumber", "1", "large", None),
            ("red onion", "1", "small", None),
            ("feta cheese", "200", "g", None),
            ("Kalamata olives", "1/2", "cup", None),
            ("olive oil", "1/4", "cup", None),
            ("red wine vinegar", "2", "tbsp", None),
            ("dried oregano", "1", "tsp", None),
        ],
        tags: &["greek", "salad", "vegetarian", "healthy", "quick"],
        source_name: None,
        image: IMAGE_SALAD,
    },
    SeedRecipe {
        title: "Homemade Pizza Dough",
        description: Some("Simple pizza dough recipe that makes two 12-inch pizzas."),
        instructions: "1. Combine warm water, sugar, and yeast. Let sit 5 minutes until foamy.
2. Add olive oil and salt to yeast mixture.
3. Gradually add flour, mixing until a dough forms.
4. Knead for 8-10 minutes until smooth and elastic.
5. Place in oiled bowl, cover, and let rise 1-2 hours until doubled.
6. Punch down and divide into two balls.
7. Let rest 15 minutes before stretching into pizza bases.
8. Top as desired and bake at 475°F (245°C) for 12-15 minutes.",
        ingredients: &[
            ("warm water", "1", "cup", Some("110°F")),
            ("active dry yeast", "2.25", "tsp", None),
            ("sugar", "1", "tsp", None),
            ("olive oil", "2", "tbsp", None),
            ("salt", "1", "tsp", None),
            ("bread flour", "3", "cups", None),
        ],
        tags: &["pizza", "baking", "italian", "basics"],
        source_name: Some("King Arthur Flour"),
        image: IMAGE_PIZZA,
    },
    SeedRecipe {
        title: "Overnight Oats",
        description: Some(
            "No-cook breakfast that you prepare the night before. Endlessly customizable!",
        ),
        instructions: "1. In a jar or container, combine oats, milk, and yogurt.
2. Add maple syrup and chia seeds, stir well.
3. Cover and refrigerate overnight (at least 6 hours).
4. In the morning, stir and add more milk if too thick.
5. Top with fresh berries, sliced banana, or nuts.
6. Can be eaten cold or warmed in the microwave.",
        ingredients: &[
            ("rolled oats", "1/2", "cup", None),
            ("milk", "1/2", "cup", Some("any type")),
            ("Greek yogurt", "1/4", "cup", None),
            ("maple syrup", "1", "tbsp", None),
            ("chia seeds", "1", "tbsp", None),
            ("fresh berries", "1/2", "cup", Some("for topping")),
        ],
        tags: &["breakfast", "healthy", "meal-prep", "no-cook", "vegetarian"],
        source_name: None,
        image: IMAGE_OATS,
    },
    SeedRecipe {
        title: "Garlic Butter Shrimp",
        description: Some(
            "Quick and elegant shrimp sautéed in garlic butter. Ready in 15 minutes!",
        ),
        instructions: "1. Pat shrimp dry and season with salt, pepper, and paprika.
2. Heat olive oil in a large skillet over medium-high heat.
3. Add shrimp in a single layer, cook 2 minutes per side until pink.
4. Remove shrimp and set aside.
5. Reduce heat, add butter and garlic, cook 1 minute.
6. Add white wine and lemon juice, simmer 2 minutes.
7. Return shrimp to pan and toss to coat.
8. Garnish with parsley and serve over pasta or with crusty bread.",
        ingredients: &[
            ("large shrimp", "1", "lb", Some("peeled and deveined")),
            ("butter", "4", "tbsp", None),
            ("garlic", "6", "cloves", Some("minced")),
            ("white wine", "1/4", "cup", None),
            ("lemon juice", "2", "tbsp", None),
            ("olive oil", "2", "tbsp", None),
            ("paprika", "1/2", "tsp", None),
            ("fresh parsley", "2", "tbsp", Some("chopped")),
        ],
        tags: &["seafood", "dinner", "quick", "date-night", "low-carb"],
        source_name: None,
        image: IMAGE_SHRIMP,
    },
];

/// Upload a photo via multipart form and return its UUID
async fn upload_photo(config: &Configuration, image_data: &[u8]) -> Result<uuid::Uuid> {
    let client = reqwest::Client::new();

    let part = reqwest::multipart::Part::bytes(image_data.to_vec())
        .file_name("image.png")
        .mime_str("image/png")?;

    let form = reqwest::multipart::Form::new().part("file", part);

    let mut request = client
        .post(format!("{}/api/photos", config.base_path))
        .multipart(form);

    if let Some(ref token) = config.bearer_access_token {
        request = request.bearer_auth(token);
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Photo upload failed with status {}: {}", status, body);
    }

    #[derive(serde::Deserialize)]
    struct UploadResponse {
        id: uuid::Uuid,
    }

    let upload_response: UploadResponse = response.json().await?;
    Ok(upload_response.id)
}

pub async fn seed(server: &str, username: &str, password: &str) -> Result<()> {
    let mut config = Configuration::new();
    config.base_path = server.to_string();

    // Try to login first - if user exists, we're done
    let login_result = auth_api::login(
        &config,
        LoginRequest {
            username: username.to_string(),
            password: password.to_string(),
        },
    )
    .await;

    match login_result {
        Ok(_) => {
            println!("User '{}' already exists, skipping seed", username);
            return Ok(());
        }
        Err(_) => {
            // User doesn't exist, create them
            let signup_response = auth_api::signup(
                &config,
                SignupRequest {
                    username: username.to_string(),
                    password: password.to_string(),
                },
            )
            .await
            .context("Failed to create user")?;

            println!("Created new user: {}", username);
            config.bearer_access_token = Some(signup_response.token);
        }
    }

    // Create recipes
    println!("Creating {} sample recipes...", SAMPLE_RECIPES.len());

    for recipe in SAMPLE_RECIPES {
        // Upload image
        let photo_id = match upload_photo(&config, recipe.image).await {
            Ok(id) => Some(id),
            Err(e) => {
                println!(
                    "  Warning: Failed to upload image for {}: {}",
                    recipe.title, e
                );
                None
            }
        };

        let ingredients: Vec<Ingredient> = recipe
            .ingredients
            .iter()
            .map(|(item, amount, unit, note)| Ingredient {
                item: item.to_string(),
                amount: if amount.is_empty() {
                    None
                } else {
                    Some(Some(amount.to_string()))
                },
                unit: if unit.is_empty() {
                    None
                } else {
                    Some(Some(unit.to_string()))
                },
                note: note.map(|n| Some(n.to_string())),
            })
            .collect();

        let request = CreateRecipeRequest {
            title: recipe.title.to_string(),
            description: recipe.description.map(|d| Some(d.to_string())),
            instructions: recipe.instructions.to_string(),
            ingredients,
            tags: if recipe.tags.is_empty() {
                None
            } else {
                Some(Some(recipe.tags.iter().map(|t| t.to_string()).collect()))
            },
            source_name: recipe.source_name.map(|s| Some(s.to_string())),
            source_url: None,
            photo_ids: photo_id.map(|id| Some(vec![id])),
        };

        recipes_api::create_recipe(&config, request)
            .await
            .with_context(|| format!("Failed to create recipe: {}", recipe.title))?;

        println!("  Created: {}", recipe.title);
    }

    println!();
    println!("{}", "=".repeat(50));
    println!("SEED DATA COMPLETE");
    println!("{}", "=".repeat(50));
    println!("Username: {}", username);
    println!("Password: {}", password);
    println!("Base URL: {}", server);
    println!("{}", "=".repeat(50));

    Ok(())
}
