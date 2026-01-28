"""Tests for Paprika export functionality."""

import gzip
import json
import zipfile
from io import BytesIO

import requests

from conftest import make_ingredient
from ramekin_client.api import RecipesApi
from ramekin_client.models import CreateRecipeRequest


def test_export_single_recipe(authed_api_client, server_url):
    """Test exporting a single recipe to .paprikarecipe format."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create a recipe with Paprika fields
    create_response = recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Export Test Recipe",
            instructions="Step 1: Do the thing.\nStep 2: Profit.",
            ingredients=[
                make_ingredient(item="flour", amount="2", unit="cups"),
                make_ingredient(item="sugar", amount="1", unit="cup"),
            ],
            description="A recipe to test export",
            tags=["test", "export"],
            source_name="Test Kitchen",
            source_url="https://example.com/recipe",
            servings="4 servings",
            prep_time="10 mins",
            cook_time="20 mins",
            total_time="30 mins",
            rating=5,
            difficulty="Easy",
            nutritional_info="100 calories",
            notes="Test notes",
        )
    )

    # Export the recipe via direct HTTP request
    token = client.configuration.access_token
    response = requests.get(
        f"{server_url}/api/recipes/{create_response.id}/export",
        headers={"Authorization": f"Bearer {token}"},
    )

    assert response.status_code == 200
    assert response.headers["content-type"] == "application/gzip"

    # Decompress and parse the exported data
    decompressed = gzip.decompress(response.content)
    exported = json.loads(decompressed)

    # Verify exported fields
    assert exported["name"] == "Export Test Recipe"
    assert exported["directions"] == "Step 1: Do the thing.\nStep 2: Profit."
    assert "flour" in exported["ingredients"]
    assert "sugar" in exported["ingredients"]
    assert exported["description"] == "A recipe to test export"
    assert exported["categories"] == ["test", "export"]
    assert exported["source"] == "Test Kitchen"
    assert exported["source_url"] == "https://example.com/recipe"
    assert exported["servings"] == "4 servings"
    assert exported["prep_time"] == "10 mins"
    assert exported["cook_time"] == "20 mins"
    assert exported["total_time"] == "30 mins"
    assert exported["rating"] == 5
    assert exported["difficulty"] == "Easy"
    assert exported["nutritional_info"] == "100 calories"
    assert exported["notes"] == "Test notes"
    # Should have uid and hash
    assert "uid" in exported
    assert "hash" in exported


def test_export_all_recipes(authed_api_client, server_url):
    """Test exporting all recipes to .paprikarecipes format."""
    client, user_id = authed_api_client
    recipes_api = RecipesApi(client)

    # Create multiple recipes
    titles = ["Recipe One", "Recipe Two", "Recipe Three"]
    for title in titles:
        recipes_api.create_recipe(
            CreateRecipeRequest(
                title=title,
                instructions=f"Instructions for {title}",
                ingredients=[],
            )
        )

    # Export all recipes via direct HTTP request
    token = client.configuration.access_token
    response = requests.get(
        f"{server_url}/api/recipes/export",
        headers={"Authorization": f"Bearer {token}"},
    )

    assert response.status_code == 200
    assert response.headers["content-type"] == "application/zip"

    # Parse the ZIP archive
    zip_buffer = BytesIO(response.content)
    with zipfile.ZipFile(zip_buffer, "r") as zf:
        # Should have 3 .paprikarecipe files
        assert len(zf.namelist()) == 3
        for name in zf.namelist():
            assert name.endswith(".paprikarecipe")

        # Verify each recipe can be extracted and parsed
        exported_titles = []
        for name in zf.namelist():
            with zf.open(name) as f:
                decompressed = gzip.decompress(f.read())
                recipe = json.loads(decompressed)
                exported_titles.append(recipe["name"])

        # All titles should be present
        for title in titles:
            assert title in exported_titles


def test_export_recipe_not_found(authed_api_client, server_url):
    """Test exporting a non-existent recipe returns 404."""
    client, user_id = authed_api_client

    token = client.configuration.access_token
    response = requests.get(
        f"{server_url}/api/recipes/00000000-0000-0000-0000-000000000000/export",
        headers={"Authorization": f"Bearer {token}"},
    )

    assert response.status_code == 404


def test_export_requires_auth(server_url):
    """Test that export endpoints require authentication."""
    # Try to export without auth
    response = requests.get(f"{server_url}/api/recipes/export")
    assert response.status_code == 401

    response = requests.get(
        f"{server_url}/api/recipes/00000000-0000-0000-0000-000000000000/export"
    )
    assert response.status_code == 401


def test_export_empty_user(authed_api_client, server_url):
    """Test exporting when user has no recipes returns empty ZIP."""
    client, user_id = authed_api_client

    token = client.configuration.access_token
    response = requests.get(
        f"{server_url}/api/recipes/export",
        headers={"Authorization": f"Bearer {token}"},
    )

    assert response.status_code == 200

    # Parse the ZIP archive - should be valid but empty
    zip_buffer = BytesIO(response.content)
    with zipfile.ZipFile(zip_buffer, "r") as zf:
        assert len(zf.namelist()) == 0
