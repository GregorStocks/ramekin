#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "urllib3",
#     "python-dateutil",
#     "pydantic",
#     "typing-extensions",
#     "cairosvg",
# ]
# ///
"""
Seed data script for development/testing.
Creates a test user with sample recipes.

Usage:
    uv run scripts/seed_data.py [--base-url URL]

The script will output the credentials for the created user.
"""

import argparse
import os
import sys

# Add the generated client to the path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "tests", "generated"))

from ramekin_client import ApiClient, Configuration
from ramekin_client.api import AuthApi, PhotosApi, RecipesApi
from ramekin_client.models import (
    CreateRecipeRequest,
    Ingredient,
    LoginRequest,
    SignupRequest,
)
import cairosvg

# Sample recipes with realistic data
SAMPLE_RECIPES = [
    {
        "title": "Classic Spaghetti Carbonara",
        "description": "A rich and creamy Italian pasta dish with eggs, cheese, and pancetta.",
        "instructions": """1. Bring a large pot of salted water to boil and cook spaghetti until al dente.
2. While pasta cooks, cut pancetta into small cubes and fry until crispy.
3. In a bowl, whisk together eggs, grated Pecorino Romano, and black pepper.
4. When pasta is done, reserve 1 cup pasta water, then drain.
5. Working quickly, add hot pasta to the pancetta pan (off heat).
6. Pour egg mixture over pasta and toss vigorously to create a creamy sauce.
7. Add pasta water as needed to reach desired consistency.
8. Serve immediately with extra cheese and black pepper.""",
        "ingredients": [
            {"item": "spaghetti", "amount": "400", "unit": "g"},
            {"item": "pancetta or guanciale", "amount": "200", "unit": "g"},
            {"item": "eggs", "amount": "4", "unit": "large"},
            {"item": "Pecorino Romano", "amount": "100", "unit": "g", "note": "freshly grated"},
            {"item": "black pepper", "amount": "2", "unit": "tsp", "note": "freshly ground"},
            {"item": "salt", "amount": "", "unit": "", "note": "for pasta water"},
        ],
        "tags": ["italian", "pasta", "dinner", "quick"],
        "source_name": "Serious Eats",
        "image": "pasta.svg",
    },
    {
        "title": "Chicken Tikka Masala",
        "description": "Tender chicken pieces in a creamy, spiced tomato sauce. A British-Indian classic.",
        "instructions": """1. Marinate chicken in yogurt, garam masala, cumin, and salt for at least 2 hours.
2. Grill or broil marinated chicken until charred and cooked through.
3. In a large pan, sauté onions until golden, then add garlic and ginger.
4. Add tomato puree, cream, and spices. Simmer for 15 minutes.
5. Cut grilled chicken into bite-sized pieces and add to the sauce.
6. Simmer together for 10 minutes to let flavors meld.
7. Garnish with fresh cilantro and serve with basmati rice or naan.""",
        "ingredients": [
            {"item": "chicken thighs", "amount": "800", "unit": "g", "note": "boneless, skinless"},
            {"item": "yogurt", "amount": "1", "unit": "cup"},
            {"item": "garam masala", "amount": "2", "unit": "tbsp"},
            {"item": "cumin", "amount": "1", "unit": "tsp"},
            {"item": "onion", "amount": "2", "unit": "large", "note": "diced"},
            {"item": "garlic", "amount": "4", "unit": "cloves", "note": "minced"},
            {"item": "ginger", "amount": "2", "unit": "inch", "note": "grated"},
            {"item": "tomato puree", "amount": "400", "unit": "g"},
            {"item": "heavy cream", "amount": "1", "unit": "cup"},
            {"item": "cilantro", "amount": "", "unit": "", "note": "for garnish"},
        ],
        "tags": ["indian", "chicken", "dinner", "spicy"],
        "image": "chicken.svg",
    },
    {
        "title": "Banana Bread",
        "description": "Moist and delicious banana bread, perfect for using up overripe bananas.",
        "instructions": """1. Preheat oven to 350°F (175°C). Grease a 9x5 inch loaf pan.
2. Mash bananas in a large bowl until smooth.
3. Mix in melted butter, then sugar, egg, and vanilla.
4. Stir in baking soda and salt, then fold in flour until just combined.
5. Pour batter into prepared pan.
6. Bake for 55-65 minutes until a toothpick comes out clean.
7. Let cool in pan for 10 minutes, then remove to wire rack.""",
        "ingredients": [
            {"item": "ripe bananas", "amount": "3", "unit": "large"},
            {"item": "butter", "amount": "1/3", "unit": "cup", "note": "melted"},
            {"item": "sugar", "amount": "3/4", "unit": "cup"},
            {"item": "egg", "amount": "1", "unit": "large"},
            {"item": "vanilla extract", "amount": "1", "unit": "tsp"},
            {"item": "baking soda", "amount": "1", "unit": "tsp"},
            {"item": "salt", "amount": "1/4", "unit": "tsp"},
            {"item": "all-purpose flour", "amount": "1.5", "unit": "cups"},
        ],
        "tags": ["baking", "breakfast", "dessert", "easy"],
        "source_name": "Grandma's Recipe Box",
        "image": "bread.svg",
    },
    {
        "title": "Thai Green Curry",
        "description": "Aromatic and creamy Thai curry with vegetables and your choice of protein.",
        "instructions": """1. Heat oil in a wok or large pan over medium-high heat.
2. Add green curry paste and fry for 1 minute until fragrant.
3. Add coconut milk and bring to a simmer.
4. Add chicken (or tofu) and cook until nearly done.
5. Add vegetables and simmer until tender-crisp.
6. Season with fish sauce, palm sugar, and lime juice.
7. Stir in Thai basil leaves just before serving.
8. Serve over jasmine rice.""",
        "ingredients": [
            {"item": "green curry paste", "amount": "3", "unit": "tbsp"},
            {"item": "coconut milk", "amount": "400", "unit": "ml"},
            {"item": "chicken breast", "amount": "500", "unit": "g", "note": "sliced"},
            {"item": "Thai eggplant", "amount": "1", "unit": "cup", "note": "quartered"},
            {"item": "bamboo shoots", "amount": "1/2", "unit": "cup"},
            {"item": "fish sauce", "amount": "2", "unit": "tbsp"},
            {"item": "palm sugar", "amount": "1", "unit": "tbsp"},
            {"item": "Thai basil", "amount": "1", "unit": "cup"},
            {"item": "lime", "amount": "1", "unit": "", "note": "juiced"},
        ],
        "tags": ["thai", "curry", "dinner", "spicy", "gluten-free"],
        "image": "curry.svg",
    },
    {
        "title": "Classic Beef Tacos",
        "description": "Simple and flavorful ground beef tacos with all the fixings.",
        "instructions": """1. Brown ground beef in a skillet over medium-high heat.
2. Drain excess fat, then add taco seasoning and water.
3. Simmer for 5-10 minutes until sauce thickens.
4. Warm taco shells according to package directions.
5. Fill shells with seasoned beef.
6. Top with shredded cheese, lettuce, tomatoes, and sour cream.
7. Squeeze fresh lime juice over tacos before serving.""",
        "ingredients": [
            {"item": "ground beef", "amount": "1", "unit": "lb"},
            {"item": "taco seasoning", "amount": "1", "unit": "packet"},
            {"item": "taco shells", "amount": "12", "unit": ""},
            {"item": "cheddar cheese", "amount": "1", "unit": "cup", "note": "shredded"},
            {"item": "lettuce", "amount": "2", "unit": "cups", "note": "shredded"},
            {"item": "tomatoes", "amount": "2", "unit": "", "note": "diced"},
            {"item": "sour cream", "amount": "1/2", "unit": "cup"},
            {"item": "lime", "amount": "1", "unit": ""},
        ],
        "tags": ["mexican", "dinner", "quick", "family-friendly"],
        "image": "tacos.svg",
    },
    {
        "title": "Mushroom Risotto",
        "description": "Creamy Italian rice dish with savory mushrooms and Parmesan cheese.",
        "instructions": """1. Heat broth in a saucepan and keep warm over low heat.
2. Sauté mushrooms in butter until golden, set aside.
3. In a large pan, sauté shallots in olive oil until soft.
4. Add rice and toast for 2 minutes, stirring constantly.
5. Add wine and stir until absorbed.
6. Add warm broth one ladle at a time, stirring until each is absorbed.
7. Continue for 18-20 minutes until rice is creamy but al dente.
8. Stir in mushrooms, butter, and Parmesan. Season to taste.""",
        "ingredients": [
            {"item": "arborio rice", "amount": "1.5", "unit": "cups"},
            {"item": "vegetable broth", "amount": "6", "unit": "cups"},
            {"item": "mixed mushrooms", "amount": "300", "unit": "g", "note": "sliced"},
            {"item": "shallots", "amount": "2", "unit": "", "note": "minced"},
            {"item": "white wine", "amount": "1/2", "unit": "cup"},
            {"item": "butter", "amount": "3", "unit": "tbsp"},
            {"item": "Parmesan", "amount": "1/2", "unit": "cup", "note": "grated"},
            {"item": "olive oil", "amount": "2", "unit": "tbsp"},
        ],
        "tags": ["italian", "vegetarian", "dinner", "comfort-food"],
        "image": "risotto.svg",
    },
    {
        "title": "Greek Salad",
        "description": "Fresh and vibrant Mediterranean salad with feta cheese and olives.",
        "instructions": """1. Cut tomatoes into wedges and cucumber into half-moons.
2. Slice red onion into thin rings.
3. Combine vegetables in a large bowl.
4. Add olives and crumbled feta cheese.
5. Drizzle with olive oil and red wine vinegar.
6. Season with oregano, salt, and pepper.
7. Toss gently and serve immediately.""",
        "ingredients": [
            {"item": "tomatoes", "amount": "4", "unit": "large"},
            {"item": "cucumber", "amount": "1", "unit": "large"},
            {"item": "red onion", "amount": "1", "unit": "small"},
            {"item": "feta cheese", "amount": "200", "unit": "g"},
            {"item": "Kalamata olives", "amount": "1/2", "unit": "cup"},
            {"item": "olive oil", "amount": "1/4", "unit": "cup"},
            {"item": "red wine vinegar", "amount": "2", "unit": "tbsp"},
            {"item": "dried oregano", "amount": "1", "unit": "tsp"},
        ],
        "tags": ["greek", "salad", "vegetarian", "healthy", "quick"],
        "image": "salad.svg",
    },
    {
        "title": "Homemade Pizza Dough",
        "description": "Simple pizza dough recipe that makes two 12-inch pizzas.",
        "instructions": """1. Combine warm water, sugar, and yeast. Let sit 5 minutes until foamy.
2. Add olive oil and salt to yeast mixture.
3. Gradually add flour, mixing until a dough forms.
4. Knead for 8-10 minutes until smooth and elastic.
5. Place in oiled bowl, cover, and let rise 1-2 hours until doubled.
6. Punch down and divide into two balls.
7. Let rest 15 minutes before stretching into pizza bases.
8. Top as desired and bake at 475°F (245°C) for 12-15 minutes.""",
        "ingredients": [
            {"item": "warm water", "amount": "1", "unit": "cup", "note": "110°F"},
            {"item": "active dry yeast", "amount": "2.25", "unit": "tsp"},
            {"item": "sugar", "amount": "1", "unit": "tsp"},
            {"item": "olive oil", "amount": "2", "unit": "tbsp"},
            {"item": "salt", "amount": "1", "unit": "tsp"},
            {"item": "bread flour", "amount": "3", "unit": "cups"},
        ],
        "tags": ["pizza", "baking", "italian", "basics"],
        "source_name": "King Arthur Flour",
        "image": "pizza.svg",
    },
    {
        "title": "Overnight Oats",
        "description": "No-cook breakfast that you prepare the night before. Endlessly customizable!",
        "instructions": """1. In a jar or container, combine oats, milk, and yogurt.
2. Add maple syrup and chia seeds, stir well.
3. Cover and refrigerate overnight (at least 6 hours).
4. In the morning, stir and add more milk if too thick.
5. Top with fresh berries, sliced banana, or nuts.
6. Can be eaten cold or warmed in the microwave.""",
        "ingredients": [
            {"item": "rolled oats", "amount": "1/2", "unit": "cup"},
            {"item": "milk", "amount": "1/2", "unit": "cup", "note": "any type"},
            {"item": "Greek yogurt", "amount": "1/4", "unit": "cup"},
            {"item": "maple syrup", "amount": "1", "unit": "tbsp"},
            {"item": "chia seeds", "amount": "1", "unit": "tbsp"},
            {"item": "fresh berries", "amount": "1/2", "unit": "cup", "note": "for topping"},
        ],
        "tags": ["breakfast", "healthy", "meal-prep", "no-cook", "vegetarian"],
        "image": "oats.svg",
    },
    {
        "title": "Garlic Butter Shrimp",
        "description": "Quick and elegant shrimp sautéed in garlic butter. Ready in 15 minutes!",
        "instructions": """1. Pat shrimp dry and season with salt, pepper, and paprika.
2. Heat olive oil in a large skillet over medium-high heat.
3. Add shrimp in a single layer, cook 2 minutes per side until pink.
4. Remove shrimp and set aside.
5. Reduce heat, add butter and garlic, cook 1 minute.
6. Add white wine and lemon juice, simmer 2 minutes.
7. Return shrimp to pan and toss to coat.
8. Garnish with parsley and serve over pasta or with crusty bread.""",
        "ingredients": [
            {"item": "large shrimp", "amount": "1", "unit": "lb", "note": "peeled and deveined"},
            {"item": "butter", "amount": "4", "unit": "tbsp"},
            {"item": "garlic", "amount": "6", "unit": "cloves", "note": "minced"},
            {"item": "white wine", "amount": "1/4", "unit": "cup"},
            {"item": "lemon juice", "amount": "2", "unit": "tbsp"},
            {"item": "olive oil", "amount": "2", "unit": "tbsp"},
            {"item": "paprika", "amount": "1/2", "unit": "tsp"},
            {"item": "fresh parsley", "amount": "2", "unit": "tbsp", "note": "chopped"},
        ],
        "tags": ["seafood", "dinner", "quick", "date-night", "low-carb"],
        "image": "shrimp.svg",
    },
]

TEST_USERNAME = "t"
TEST_PASSWORD = "t"


def create_ingredient(data: dict) -> Ingredient:
    """Create an Ingredient object from dict data."""
    return Ingredient(
        item=data["item"],
        amount=data.get("amount", ""),
        unit=data.get("unit", ""),
        note=data.get("note"),
    )


def load_image(image_name: str) -> bytes:
    """Load an SVG image and convert to PNG bytes."""
    script_dir = os.path.dirname(__file__)
    svg_path = os.path.join(script_dir, "seed_images", image_name)
    return cairosvg.svg2png(url=svg_path, output_width=400, output_height=400)


def seed_data(base_url: str) -> dict:
    """
    Create a test user with sample recipes (idempotent).
    Returns dict with user credentials and info.
    """
    config = Configuration(host=base_url)

    with ApiClient(config) as client:
        auth_api = AuthApi(client)

        # Try to login first - if user exists, we're done
        try:
            auth_api.login(
                LoginRequest(username=TEST_USERNAME, password=TEST_PASSWORD)
            )
            print(f"Test user '{TEST_USERNAME}' already exists, skipping seed")
            return {
                "username": TEST_USERNAME,
                "password": TEST_PASSWORD,
                "base_url": base_url,
            }
        except Exception:
            # User doesn't exist, create them
            response = auth_api.signup(
                SignupRequest(username=TEST_USERNAME, password=TEST_PASSWORD)
            )
            print(f"Created new user: {TEST_USERNAME}")
            token = response.token
            user_id = response.user_id

    # Create recipes for new user
    config.access_token = token
    with ApiClient(config) as client:
        recipes_api = RecipesApi(client)
        photos_api = PhotosApi(client)

        print(f"Creating {len(SAMPLE_RECIPES)} sample recipes...")
        for recipe_data in SAMPLE_RECIPES:
            # Upload image if present
            photo_ids = []
            if "image" in recipe_data:
                try:
                    image_bytes = load_image(recipe_data["image"])
                    upload_response = photos_api.upload(file=image_bytes)
                    photo_ids = [str(upload_response.id)]
                except Exception as e:
                    print(f"  Warning: Failed to upload image for {recipe_data['title']}: {e}")

            request = CreateRecipeRequest(
                title=recipe_data["title"],
                description=recipe_data.get("description"),
                instructions=recipe_data["instructions"],
                ingredients=[create_ingredient(ing) for ing in recipe_data["ingredients"]],
                tags=recipe_data.get("tags"),
                source_name=recipe_data.get("source_name"),
                source_url=recipe_data.get("source_url"),
                photo_ids=photo_ids if photo_ids else None,
            )
            recipes_api.create_recipe(request)
            print(f"  Created: {recipe_data['title']}")

    print()
    print("=" * 50)
    print("SEED DATA COMPLETE")
    print("=" * 50)
    print(f"Username: {TEST_USERNAME}")
    print(f"Password: {TEST_PASSWORD}")
    print(f"Base URL: {base_url}")
    print("=" * 50)

    return {
        "username": TEST_USERNAME,
        "password": TEST_PASSWORD,
        "user_id": user_id,
        "base_url": base_url,
    }


def main():
    parser = argparse.ArgumentParser(description="Seed development data")
    parser.add_argument(
        "--base-url",
        default=os.environ.get("API_BASE_URL", "http://localhost:3000"),
        help="API base URL (default: http://localhost:3000)",
    )
    args = parser.parse_args()

    seed_data(args.base_url)


if __name__ == "__main__":
    main()
