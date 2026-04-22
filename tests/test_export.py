"""Tests for Paprika export functionality."""

import base64
import gzip
import json
import zipfile
from io import BytesIO

import requests
from PIL import Image

from conftest import make_ingredient
from ramekin_client.api import PhotosApi, RecipesApi
from ramekin_client.models import CreateRecipeRequest

# Must match server::photos::processing::EXPORT_PHOTO_MAX_DIMENSION
EXPORT_PHOTO_MAX_DIMENSION = 1600


def _export_zip_contents(server_url, client):
    """Fetch /api/recipes/export and parse into a dict of filename → recipe JSON."""
    token = client.configuration.access_token
    response = requests.get(
        f"{server_url}/api/recipes/export",
        headers={"Authorization": f"Bearer {token}"},
    )
    assert response.status_code == 200
    assert response.headers["content-type"] == "application/zip"
    zip_buffer = BytesIO(response.content)
    out = {}
    with zipfile.ZipFile(zip_buffer, "r") as zf:
        for name in zf.namelist():
            with zf.open(name) as f:
                out[name] = json.loads(gzip.decompress(f.read()))
    return out


def test_export_downscales_photos(authed_api_client, server_url):
    """Photos embedded in a paprikarecipes export must be resized so the
    longest side is <= EXPORT_PHOTO_MAX_DIMENSION (1600px).

    The server originally embedded full-resolution photos (up to 10MB each)
    as base64, which caused the server to OOM on bulk export of a real
    library. This test guards the downscale path in
    server::api::recipes::export::convert_to_paprika.
    """
    client, _user_id = authed_api_client
    recipes_api = RecipesApi(client)
    photos_api = PhotosApi(client)

    # 2400x1800 is well above the 1600 cap on the longest side.
    large_jpeg = _make_large_jpeg(2400, 1800)
    photo_id = str(photos_api.upload(file=("big.jpg", large_jpeg)).id)

    recipes_api.create_recipe(
        CreateRecipeRequest(
            title="Recipe With Big Photo",
            instructions="Cook it",
            ingredients=[make_ingredient(item="flour", amount="2", unit="cups")],
            photo_ids=[photo_id],
        )
    )

    contents = _export_zip_contents(server_url, client)
    assert len(contents) == 1
    recipe = next(iter(contents.values()))
    assert recipe["name"] == "Recipe With Big Photo"
    assert len(recipe["photos"]) == 1
    photo_b64 = recipe["photos"][0]["data"]
    photo_bytes = base64.b64decode(photo_b64)

    # Resized output is JPEG and fits within the export cap.
    assert photo_bytes[:3] == b"\xff\xd8\xff", "resized photo should be JPEG"
    resized = Image.open(BytesIO(photo_bytes))
    assert max(resized.size) <= EXPORT_PHOTO_MAX_DIMENSION, (
        f"resized photo {resized.size} exceeds export cap {EXPORT_PHOTO_MAX_DIMENSION}"
    )
    # And it should actually be smaller than the original — if it were a
    # no-op, this whole exercise would have been pointless.
    assert len(photo_bytes) < len(large_jpeg)


def _make_large_jpeg(width: int, height: int) -> bytes:
    """Build a JPEG of the given dimensions for exercising photo resizing."""
    img = Image.new("RGB", (width, height), color=(123, 45, 67))
    # Add a diagonal gradient so the JPEG doesn't compress down to ~nothing
    # and we get a realistic non-trivial file size.
    px = img.load()
    for y in range(height):
        for x in range(0, width, 32):
            px[x, y] = ((x + y) % 256, (x * 2) % 256, (y * 3) % 256)
    buf = BytesIO()
    img.save(buf, format="JPEG", quality=90)
    return buf.getvalue()


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
    assert exported["categories"] == ["export", "test"]
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
