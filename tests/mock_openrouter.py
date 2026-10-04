#!/usr/bin/env python3
"""Mock OpenRouter server for testing.

Returns valid OpenAI-compatible chat completion responses.
"""

import base64
import json
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse


class SlowImageGenerationBarrier:
    def __init__(self):
        self.started = threading.Event()
        self.release = threading.Event()

    def reset(self):
        self.started.clear()
        self.release.clear()


SLOW_IMAGE_GENERATION_BARRIER = SlowImageGenerationBarrier()

# How many ingredient-name-resolver calls included each name, so tests can
# check that concurrent saves of one name pay for a single call.
INGREDIENT_NAME_CALLS = {}
# Valid answers sent per name. A batch broken by another test's failing name
# is re-asked one name at a time, so calls can exceed answers.
INGREDIENT_NAME_ANSWERS = {}
INGREDIENT_NAME_CALLS_LOCK = threading.Lock()
# Names whose resolver calls fail until a test clears them.
FAILING_INGREDIENT_NAMES = set()
# Names whose answer is held back until a test releases them, so the test can
# observe a name while it is still pending. Never held longer than this.
HELD_INGREDIENT_NAMES = set()
INGREDIENT_NAME_HOLD_LIMIT_SECONDS = 15


def mock_png_data_url():
    image_path = (
        Path(__file__).resolve().parent.parent
        / "cli"
        / "src"
        / "seed_images"
        / "bread.png"
    )
    encoded = base64.b64encode(image_path.read_bytes()).decode()
    return f"data:image/png;base64,{encoded}"


class MockOpenRouterHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        parsed = urlparse(self.path)
        if parsed.path == "/test/slow-image-generation/reset":
            SLOW_IMAGE_GENERATION_BARRIER.reset()
            self.send_response(204)
            self.end_headers()
            return

        if parsed.path == "/test/slow-image-generation/started":
            params = parse_qs(parsed.query)
            timeout = float(params.get("timeout", ["5"])[0])
            if not SLOW_IMAGE_GENERATION_BARRIER.started.wait(timeout):
                self.send_error(504, "Timed out waiting for slow image generation")
                return
            self.send_response(204)
            self.end_headers()
            return

        if parsed.path == "/test/slow-image-generation/release":
            SLOW_IMAGE_GENERATION_BARRIER.release.set()
            self.send_response(204)
            self.end_headers()
            return

        if parsed.path == "/test/ingredient-name-failure":
            params = parse_qs(parsed.query)
            name = params.get("name", [""])[0]
            with INGREDIENT_NAME_CALLS_LOCK:
                if params.get("fail", ["true"])[0] == "true":
                    FAILING_INGREDIENT_NAMES.add(name)
                else:
                    FAILING_INGREDIENT_NAMES.discard(name)
            self.send_response(204)
            self.end_headers()
            return

        if parsed.path == "/test/ingredient-name-hold":
            params = parse_qs(parsed.query)
            name = params.get("name", [""])[0]
            with INGREDIENT_NAME_CALLS_LOCK:
                if params.get("hold", ["true"])[0] == "true":
                    HELD_INGREDIENT_NAMES.add(name)
                else:
                    HELD_INGREDIENT_NAMES.discard(name)
            self.send_response(204)
            self.end_headers()
            return

        if parsed.path == "/test/ingredient-name-calls":
            name = parse_qs(parsed.query).get("name", [""])[0]
            with INGREDIENT_NAME_CALLS_LOCK:
                counts = {
                    "calls": INGREDIENT_NAME_CALLS.get(name, 0),
                    "answers": INGREDIENT_NAME_ANSWERS.get(name, 0),
                }
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(counts).encode())
            return

        # Health check endpoint
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(b'{"status": "ok"}')

    def do_POST(self):
        if self.path == "/v1/chat/completions":
            # Read request body
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)

            try:
                request = json.loads(body)
            except json.JSONDecodeError:
                self.send_error(400, "Invalid JSON")
                return

            # Per-recipe trigger keeps concurrent tests isolated while exercising
            # the real client's handling of OpenRouter authentication failures.
            if "SeedAuthFailureFixture" in json.dumps(request.get("messages", [])):
                self.send_response(401)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(
                    b'{"error":{"message":"Missing Authentication header","code":401}}'
                )
                return

            if "image" in request.get("modalities", []):
                try:
                    response = self._mock_image_generation_response(request)
                except TimeoutError as exc:
                    self.send_error(500, str(exc))
                    return
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps(response).encode())
                return

            content = self._generate_response_content(request)

            response = {
                "id": "mock-completion-id",
                "object": "chat.completion",
                "created": 1234567890,
                "model": request.get("model", "mock-model"),
                "choices": [
                    {
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": content,
                        },
                        "finish_reason": "stop",
                    }
                ],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 5,
                    "total_tokens": 15,
                },
            }

            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(response).encode())
        else:
            self.send_error(404, "Not found")

    def _mock_resolve_ingredient_names(self, all_text):
        """Resolve each item to its first candidate (an ambiguous one too).
        Names a test marked failing (/test/ingredient-name-failure) break the
        response; "estimable" gets an estimate (200 kcal/100 g, 150 g/cup,
        40 g/piece); "unknowable" answers unknown; "serving platter" is not
        food."""
        items_text = all_text.split("Items:", 1)[1].split("Respond with JSON", 1)[0]
        items = json.loads(items_text)
        with INGREDIENT_NAME_CALLS_LOCK:
            for item in items:
                INGREDIENT_NAME_CALLS[item["name"]] = (
                    INGREDIENT_NAME_CALLS.get(item["name"], 0) + 1
                )
        deadline = time.monotonic() + INGREDIENT_NAME_HOLD_LIMIT_SECONDS
        while time.monotonic() < deadline:
            with INGREDIENT_NAME_CALLS_LOCK:
                if not any(item["name"] in HELD_INGREDIENT_NAMES for item in items):
                    break
            time.sleep(0.05)
        with INGREDIENT_NAME_CALLS_LOCK:
            failing = any(item["name"] in FAILING_INGREDIENT_NAMES for item in items)
        if failing:
            return '{"resolutions": ['
        resolutions = []
        for item in items:
            if "estimable" in item["name"] and not item["ambiguous"]:
                resolutions.append(
                    {
                        "name": item["name"],
                        "answer": "estimate",
                        "kcal_per_100g": 200,
                        "grams_per_cup": 150,
                        "grams_per_piece": 40,
                    }
                )
            elif "unknowable" in item["name"] or not item["candidates"]:
                resolutions.append({"name": item["name"], "answer": "unknown"})
            elif "serving platter" in item["name"]:
                resolutions.append({"name": item["name"], "answer": "not_food"})
            else:
                resolutions.append(
                    {
                        "name": item["name"],
                        "answer": "entry",
                        "key": item["candidates"][0],
                    }
                )
        with INGREDIENT_NAME_CALLS_LOCK:
            for item in items:
                INGREDIENT_NAME_ANSWERS[item["name"]] = (
                    INGREDIENT_NAME_ANSWERS.get(item["name"], 0) + 1
                )
        return json.dumps({"resolutions": resolutions})

    def _mock_estimate_ingredient_weights(self, all_text):
        """Weigh each (food, unit): 120 g per cup, 50 g per anything else. A
        "handful" has no typical weight (null); a unit a test marked failing
        (/test/ingredient-name-failure) breaks the response. Calls and answers
        are counted under the unit."""
        items_text = all_text.split("Items:", 1)[1].split("Respond with JSON", 1)[0]
        items = json.loads(items_text)
        with INGREDIENT_NAME_CALLS_LOCK:
            for item in items:
                INGREDIENT_NAME_CALLS[item["unit"]] = (
                    INGREDIENT_NAME_CALLS.get(item["unit"], 0) + 1
                )
            failing = any(item["unit"] in FAILING_INGREDIENT_NAMES for item in items)
        if failing:
            return '{"weights": ['
        weights = []
        for item in items:
            if item["unit"] == "handful":
                grams = None
            elif item["unit"] == "cup":
                grams = 120
            else:
                grams = 50
            weights.append({"food": item["food"], "unit": item["unit"], "grams": grams})
        with INGREDIENT_NAME_CALLS_LOCK:
            for item in items:
                INGREDIENT_NAME_ANSWERS[item["unit"]] = (
                    INGREDIENT_NAME_ANSWERS.get(item["unit"], 0) + 1
                )
        return json.dumps({"weights": weights})

    def _generate_response_content(self, request):
        """Generate appropriate mock response based on the request type."""
        if "image" in request.get("modalities", []):
            return self._mock_image_generation_content()

        messages = request.get("messages", [])
        has_images = False

        # Extract text and note whether any image inputs were provided.
        all_text = ""
        for m in messages:
            content = m.get("content", "")
            if isinstance(content, str):
                all_text += " " + content
            elif isinstance(content, list):
                for part in content:
                    if not isinstance(part, dict):
                        continue
                    if part.get("type") == "text":
                        all_text += " " + part.get("text", "")
                    elif part.get("type") == "image_url":
                        has_images = True

        if "ingredient name resolver" in all_text:
            return self._mock_resolve_ingredient_names(all_text)

        if "ingredient weight estimator" in all_text:
            return self._mock_estimate_ingredient_weights(all_text)

        if "recipe modification assistant" in all_text:
            return self._mock_custom_enrich(all_text, has_images=has_images)

        if "recipe text extraction assistant" in all_text:
            if "TEXT_EXTRACTION_FAILURE" in all_text:
                return '{"raw_recipe":'
            incomplete = "TEXT_INCOMPLETE" in all_text
            return json.dumps(
                {
                    "raw_recipe": {
                        "title": ""
                        if incomplete
                        else (
                            "Force Text Enrichment Failure"
                            if "TEXT_ENRICHMENT_FAILURE" in all_text
                            else "Text Pancakes"
                        ),
                        "ingredients": (
                            "1 cup all-purpose flour\n8 oz butter\n1 cup unknowable powder"
                        ),
                        "instructions": ""
                        if incomplete
                        else "Mix ingredients.\n\nCook in a pan.",
                        "image_urls": [],
                        "servings": "4",
                        "prep_time": "10 minutes",
                        "source_name": "Family notebook",
                        "notes": "Serve warm.",
                    },
                    "warnings": (
                        ["Check the missing fields."]
                        if incomplete
                        else (
                            ["The source mentions a note that was not supplied."]
                            if "TEXT_MODEL_WARNING" in all_text
                            else []
                        )
                    ),
                }
            )

        if has_images:
            return self._mock_photo_extract()

        if "like what you'd see on a restaurant menu" in all_text:
            return self._mock_generate_description(all_text)

        if "recipe title editor" in all_text:
            return self._mock_normalize_title(all_text)

        if "Force Auto Tag Failure" in all_text:
            return '{"suggested_tags": ['

        # Default: auto-tag response
        return '{"suggested_tags": ["test-auto-tag"]}'

    def _mock_image_generation_content(self):
        return "Generated recipe photo."

    def _mock_image_generation_response(self, request):
        messages = request.get("messages", [])
        all_text = " ".join(
            content
            for message in messages
            for content in [message.get("content", "")]
            if isinstance(content, str)
        )
        if "Slow Generated Photo" in all_text:
            SLOW_IMAGE_GENERATION_BARRIER.started.set()
            if not SLOW_IMAGE_GENERATION_BARRIER.release.wait(10.0):
                raise TimeoutError("Slow image generation was never released")

        return {
            "id": "mock-image-generation-id",
            "object": "chat.completion",
            "created": 1234567890,
            "model": request.get("model", "mock-model"),
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": self._mock_image_generation_content(),
                        "images": [
                            {
                                "type": "image_url",
                                "image_url": {"url": mock_png_data_url()},
                            }
                        ],
                    },
                    "finish_reason": "stop",
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15,
            },
        }

    def _mock_custom_enrich(self, all_text, has_images=False):
        """Return a modified recipe for custom enrich requests."""
        # Try to extract the recipe JSON from the prompt
        try:
            # The recipe JSON is between "Here is the recipe:" and "Apply this change:"
            start = all_text.index("Here is the recipe:") + len("Here is the recipe:")
            end = all_text.index("Apply this change:")
            recipe_json = all_text[start:end].strip()
            recipe_json = recipe_json.replace(
                (
                    "If reference photos are attached to this message, use them as "
                    "additional context when they help."
                ),
                "",
            ).strip()
            recipe = json.loads(recipe_json)
            # Apply a visible modification so tests can tell whether images were passed.
            prefix = "[Modified with Photo] " if has_images else "[Modified] "
            recipe["title"] = prefix + recipe.get("title", "")
            return json.dumps(recipe)
        except (ValueError, json.JSONDecodeError):
            # Fallback: return a minimal valid recipe
            return json.dumps(
                {
                    "title": (
                        "[Modified with Photo] Test Recipe"
                        if has_images
                        else "[Modified] Test Recipe"
                    ),
                    "instructions": "Modified instructions.",
                    "ingredients": [],
                    "tags": [],
                }
            )

    def _mock_generate_description(self, all_text):
        """Return a mock generated description."""
        return json.dumps({"description": "A delicious test recipe."})

    def _mock_normalize_title(self, all_text):
        """Return a mock normalized title."""
        if "Force Text Enrichment Failure" in all_text:
            return '{"normalized_title":'
        # Try to extract the title from the prompt
        try:
            start = all_text.index("- Title: ") + len("- Title: ")
            end = all_text.index("\n", start)
            title = all_text[start:end].strip()
            return json.dumps({"normalized_title": title})
        except ValueError:
            return json.dumps({"normalized_title": "Normalized Recipe"})

    def _mock_photo_extract(self):
        """Return a mock recipe extracted from photos."""
        return json.dumps(
            {
                "title": "Photo Imported Recipe",
                "description": "A recipe extracted from a photo",
                "ingredients": "1 cup flour\n2 eggs\n1/2 cup sugar",
                "instructions": (
                    "Mix all ingredients together.\n\nBake at 350F for 30 minutes."
                ),
                "servings": "4 servings",
                "prep_time": "10 minutes",
                "cook_time": "30 minutes",
                "total_time": "40 minutes",
                "notes": None,
            }
        )

    def log_message(self, format, *args):
        pass


def main():
    if len(sys.argv) < 2:
        print("Error: Port argument required", file=sys.stderr)
        print("Usage: python mock_openrouter.py <port>", file=sys.stderr)
        sys.exit(1)
    port = int(sys.argv[1])
    server = ThreadingHTTPServer(("", port), MockOpenRouterHandler)
    print(f"Mock OpenRouter server running on port {port}", file=sys.stderr)
    server.serve_forever()


if __name__ == "__main__":
    main()
