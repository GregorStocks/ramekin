#!/usr/bin/env python3
"""Mock OpenRouter server for testing.

Returns valid OpenAI-compatible chat completion responses.
"""

import base64
import json
import socket
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
# is re-asked in halves, so calls can exceed answers.
INGREDIENT_NAME_ANSWERS = {}
INGREDIENT_NAME_CALLS_LOCK = threading.Lock()
# Names whose resolver calls fail until a test clears them, or (with a count)
# for that many more calls that include them.
FAILING_INGREDIENT_NAMES = {}
# Names whose resolver calls get an HTTP 503, as in a provider outage, until a
# test clears them. Counted as calls.
DOWN_INGREDIENT_NAMES = set()
# Names whose answer is held back until a test releases them, so the test can
# observe a name while it is still pending. Never held longer than this.
HELD_INGREDIENT_NAMES = set()
INGREDIENT_NAME_HOLD_LIMIT_SECONDS = 15
# Description requests whose prompt contains one of these markers are held
# until a test releases the marker (bounded by the same limit), so a test can
# act while a scrape job's description call is in flight.
HELD_DESCRIPTION_MARKERS = set()


def mock_png_base64():
    image_path = (
        Path(__file__).resolve().parent.parent
        / "cli"
        / "src"
        / "seed_images"
        / "bread.png"
    )
    return base64.b64encode(image_path.read_bytes()).decode()


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
                    times = params.get("times", [None])[0]
                    FAILING_INGREDIENT_NAMES[name] = int(times) if times else None
                else:
                    FAILING_INGREDIENT_NAMES.pop(name, None)
            self.send_response(204)
            self.end_headers()
            return

        if parsed.path == "/test/ingredient-name-outage":
            params = parse_qs(parsed.query)
            name = params.get("name", [""])[0]
            with INGREDIENT_NAME_CALLS_LOCK:
                if params.get("down", ["true"])[0] == "true":
                    DOWN_INGREDIENT_NAMES.add(name)
                else:
                    DOWN_INGREDIENT_NAMES.discard(name)
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

        if parsed.path == "/test/description-hold":
            params = parse_qs(parsed.query)
            marker = params.get("marker", [""])[0]
            with INGREDIENT_NAME_CALLS_LOCK:
                if params.get("hold", ["true"])[0] == "true":
                    HELD_DESCRIPTION_MARKERS.add(marker)
                else:
                    HELD_DESCRIPTION_MARKERS.discard(marker)
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
        if self.path == "/v1/images":
            content_length = int(self.headers.get("Content-Length", 0))
            try:
                request = json.loads(self.rfile.read(content_length))
            except json.JSONDecodeError:
                self.send_error(400, "Invalid JSON")
                return
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

            if self._ingredient_names_down(request):
                self.send_error(503, "Provider unavailable")
                return

            if request.get("tools"):
                message, finish_reason = self._mock_edit_recipe_turn(request)
            else:
                message = {
                    "role": "assistant",
                    "content": self._generate_response_content(request),
                }
                finish_reason = "stop"

            response = {
                "id": "mock-completion-id",
                "object": "chat.completion",
                "created": 1234567890,
                "model": request.get("model", "mock-model"),
                "choices": [
                    {
                        "index": 0,
                        "message": message,
                        "finish_reason": finish_reason,
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

    def _ingredient_names_down(self, request):
        """Whether this is a resolver call that includes a name a test took
        down (/test/ingredient-name-outage); counts the call if so."""
        text = json.dumps(request.get("messages", []))
        if "ingredient name resolver" not in text:
            return False
        with INGREDIENT_NAME_CALLS_LOCK:
            down = [name for name in DOWN_INGREDIENT_NAMES if name in text]
            for name in down:
                INGREDIENT_NAME_CALLS[name] = INGREDIENT_NAME_CALLS.get(name, 0) + 1
        return bool(down)

    def _mock_resolve_ingredient_names(self, all_text):
        """Resolve each item to its first candidate (an ambiguous one too).
        Names a test marked failing (/test/ingredient-name-failure) break the
        response; "estimable" gets an estimate (200 kcal/100 g, 150 g/cup,
        40 g/piece, Snacks); "unknowable" answers unknown; "serving platter" is not
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
            failing = [
                i["name"] for i in items if i["name"] in FAILING_INGREDIENT_NAMES
            ]
            for name in failing:
                remaining = FAILING_INGREDIENT_NAMES[name]
                if remaining is not None:
                    if remaining <= 1:
                        del FAILING_INGREDIENT_NAMES[name]
                    else:
                        FAILING_INGREDIENT_NAMES[name] = remaining - 1
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
                        "shopping_category": "Snacks",
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

    def _generate_response_content(self, request):
        """Generate appropriate mock response based on the request type."""

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
                            "Force Text Normalize Failure"
                            if "TEXT_NORMALIZE_FAILURE" in all_text
                            else "Text Pancakes"
                        ),
                        "ingredients": (
                            "1 cup all-purpose flour\n8 oz butter\n"
                            "1 cup unknowable powder"
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

    def _mock_image_generation_response(self, request):
        """An images-endpoint answer: one base64 PNG and its cost."""
        if "Slow Generated Photo" in request.get("prompt", ""):
            SLOW_IMAGE_GENERATION_BARRIER.started.set()
            if not SLOW_IMAGE_GENERATION_BARRIER.release.wait(10.0):
                raise TimeoutError("Slow image generation was never released")

        return {
            "created": 1234567890,
            "data": [
                {
                    "b64_json": mock_png_base64(),
                    "media_type": "image/png",
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15,
                "cost": 0.0,
            },
        }

    def _mock_edit_recipe_turn(self, request):
        """One turn of an Edit with AI tool loop: first prefix the title and
        look an ingredient up, then finish once those tools have answered.
        The prefix says whether photos came along, so tests can tell."""
        messages = request.get("messages", [])
        has_images = any(
            isinstance(m.get("content"), list)
            and any(
                isinstance(part, dict) and part.get("type") == "image_url"
                for part in m["content"]
            )
            for m in messages
        )
        prompt = json.dumps(messages)
        tool_results = [m for m in messages if m.get("role") == "tool"]
        if "into the house style" in prompt:
            # House style leaves recipes alone, so imports in other tests
            # keep their steps, except one titled for the house style test,
            # which gets a prep step.
            if "House Style Test" in prompt and not tool_results:
                calls = [("add_step", {"after": 0, "text": "Chop the onion."})]
            else:
                calls = [("finish", {"summary": "Rewrote it in house style."})]
        elif "like my other" in prompt and not tool_results:
            # Search the user's recipes by an ingredient, and report what
            # came back so tests can see the search.
            calls = [("search_recipes", {"query": "tahini"})]
        elif "like my other" in prompt:
            # Cache breakpoints turn the latest message's text into parts.
            content = tool_results[-1].get("content", "")
            if isinstance(content, list):
                content = "".join(part.get("text", "") for part in content)
            found = content
            calls = [("finish", {"summary": "Found: " + found.split("\n")[0]})]
        elif tool_results:
            calls = [("finish", {"summary": "Marked the title as modified."})]
        else:
            title = "Test Recipe"
            marker = "Title: "
            if marker in prompt:
                start = prompt.index(marker) + len(marker)
                title = prompt[start : prompt.index("\\n", start)]
            prefix = "[Modified with Photo] " if has_images else "[Modified] "
            calls = [
                ("set_field", {"field": "title", "value": prefix + title}),
                ("lookup_ingredient", {"name": "flour"}),
            ]
        tool_calls = [
            {
                "id": f"mock-call-{len(messages)}-{index}",
                "type": "function",
                "function": {"name": name, "arguments": json.dumps(args)},
            }
            for index, (name, args) in enumerate(calls)
        ]
        return {"role": "assistant", "content": None, "tool_calls": tool_calls}, (
            "tool_calls"
        )

    def _mock_generate_description(self, all_text):
        """Return a mock generated description, once no held marker
        (/test/description-hold) appears in the prompt."""
        deadline = time.monotonic() + INGREDIENT_NAME_HOLD_LIMIT_SECONDS
        while time.monotonic() < deadline:
            with INGREDIENT_NAME_CALLS_LOCK:
                if not any(marker in all_text for marker in HELD_DESCRIPTION_MARKERS):
                    break
            time.sleep(0.05)
        return json.dumps({"description": "A delicious test recipe."})

    def _mock_normalize_title(self, all_text):
        """Return a mock normalized title."""
        if "Force Text Normalize Failure" in all_text:
            return '{"normalized_title":'
        # Try to extract the title from the prompt
        try:
            start = all_text.index("- Title: ") + len("- Title: ")
            end = all_text.index("\n", start)
            # Collapse whitespace runs so a test can provoke a changed title.
            title = " ".join(all_text[start:end].split())
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
                "nutritional_info": "Per serving: Cal 410 • Total Fat 10g",
                "notes": None,
            }
        )

    def log_message(self, format, *args):
        pass


class DualStackHTTPServer(ThreadingHTTPServer):
    address_family = socket.AF_INET6

    def server_bind(self):
        self.socket.setsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY, 0)
        super().server_bind()


def make_server(port):
    """Listen on ::1 as well as 127.0.0.1. A `localhost` client that tries ::1
    first and finds no listener can occasionally connect to itself when the
    port is in the ephemeral range, and read back its own request."""
    try:
        return DualStackHTTPServer(("::", port), MockOpenRouterHandler)
    except OSError:
        return ThreadingHTTPServer(("", port), MockOpenRouterHandler)


def main():
    if len(sys.argv) < 2:
        print("Error: Port argument required", file=sys.stderr)
        print("Usage: python mock_openrouter.py <port>", file=sys.stderr)
        sys.exit(1)
    port = int(sys.argv[1])
    server = make_server(port)
    print(f"Mock OpenRouter server running on port {port}", file=sys.stderr)
    server.serve_forever()


if __name__ == "__main__":
    main()
