#!/usr/bin/env python3
"""Mock OpenRouter server for testing.

Returns valid OpenAI-compatible chat completion responses.
"""

import base64
import json
import sys
import time
from http.server import HTTPServer, BaseHTTPRequestHandler
from pathlib import Path


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

            if "image" in request.get("modalities", []):
                response = self._mock_image_generation_response(request)
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

        if "recipe modification assistant" in all_text:
            return self._mock_custom_enrich(all_text, has_images=has_images)

        if has_images:
            return self._mock_photo_extract()

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
            time.sleep(1.0)

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
    server = HTTPServer(("", port), MockOpenRouterHandler)
    print(f"Mock OpenRouter server running on port {port}", file=sys.stderr)
    server.serve_forever()


if __name__ == "__main__":
    main()
